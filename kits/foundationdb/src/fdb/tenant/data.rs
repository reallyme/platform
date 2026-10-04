// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Application key access inside a FoundationDB tenant.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};

use foundationdb::{
    RangeOption, Transaction,
    future::{FdbSlice, FdbValues},
    options::{MutationType, TransactionOption},
};
use thiserror::Error;

const RESERVED_METADATA_START: &[u8] = b"__meta";
const RESERVED_METADATA_END: &[u8] = b"__metb";
const MAX_TENANT_KEY_BYTES: usize = 10_000;
const MAX_TENANT_VALUE_BYTES: usize = 100_000;
const MAX_RANGE_RESULTS: usize = 64;
const MAX_RANGE_TARGET_BYTES: usize = 1_000_000;
const MAX_TRANSACTION_SIZE_LIMIT: i32 = 10_000_000;
const VERSIONSTAMP_BYTES: usize = 10;
const VERSIONSTAMP_OFFSET_BYTES: usize = 4;
const MIN_STABLE_KEY_PREFIX_BYTES: usize = 6;

/// A finite reason for rejecting an application key or range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TenantDataAccessErrorReason {
    /// Keys cannot be empty.
    #[error("tenant data key is empty")]
    EmptyKey,
    /// FoundationDB cannot accept a key above its size limit.
    #[error("tenant data key exceeds the size limit")]
    KeyTooLong,
    /// Kit metadata and FoundationDB system keys are outside application access.
    #[error("tenant data key is reserved")]
    ReservedKey,
    /// FoundationDB cannot accept a value above its size limit.
    #[error("tenant data value exceeds the size limit")]
    ValueTooLong,
    /// A range must have a non-empty, increasing interval.
    #[error("tenant data range bounds are invalid")]
    InvalidRange,
    /// A range could read or clear the kit's metadata keys.
    #[error("tenant data range overlaps reserved metadata")]
    ReservedRange,
    /// Range reads and transaction options must have bounded limits.
    #[error("tenant data limit is invalid")]
    InvalidRangeLimit,
    /// A mutation is prohibited by the transaction policy or key boundary.
    #[error("tenant data mutation is prohibited")]
    KeyChangingMutation,
}

/// Validated cluster-side transaction byte ceiling.
#[derive(Clone, Copy)]
pub struct TenantTransactionSizeLimit(i32);

impl TenantTransactionSizeLimit {
    /// Validates the positive FoundationDB transaction size limit.
    pub fn new(bytes: i32) -> Result<Self, TenantDataAccessErrorReason> {
        if !(1..=MAX_TRANSACTION_SIZE_LIMIT).contains(&bytes) {
            return Err(TenantDataAccessErrorReason::InvalidRangeLimit);
        }
        Ok(Self(bytes))
    }
}

/// Validated maximum response bytes for one range request.
#[derive(Clone, Copy)]
pub struct TenantDataRangeTargetBytes(NonZeroUsize);

impl TenantDataRangeTargetBytes {
    /// Rejects unbounded or empty response targets.
    pub fn new(bytes: usize) -> Result<Self, TenantDataAccessErrorReason> {
        let bytes = NonZeroUsize::new(bytes)
            .filter(|bytes| bytes.get() <= MAX_RANGE_TARGET_BYTES)
            .ok_or(TenantDataAccessErrorReason::InvalidRangeLimit)?;
        Ok(Self(bytes))
    }
}

/// A borrowed application key validated against the kit's reserved namespace.
#[derive(Clone, Copy)]
pub struct TenantDataKey<'a> {
    bytes: &'a [u8],
}

impl<'a> TenantDataKey<'a> {
    /// Validates a tenant-relative key before it reaches a transaction.
    pub fn new(bytes: &'a [u8]) -> Result<Self, TenantDataAccessErrorReason> {
        if bytes.is_empty() {
            return Err(TenantDataAccessErrorReason::EmptyKey);
        }
        if bytes.len() > MAX_TENANT_KEY_BYTES {
            return Err(TenantDataAccessErrorReason::KeyTooLong);
        }
        if (RESERVED_METADATA_START..RESERVED_METADATA_END).contains(&bytes) || bytes[0] == 0xff {
            return Err(TenantDataAccessErrorReason::ReservedKey);
        }
        Ok(Self { bytes })
    }

    /// Returns the validated tenant-relative bytes.
    pub fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// A validated half-open range that cannot include metadata keys.
pub struct TenantDataRange<'a> {
    begin: TenantDataKey<'a>,
    end: TenantDataKey<'a>,
}

impl<'a> TenantDataRange<'a> {
    /// Checks both bounds and the complete interval between them.
    pub fn new(begin: &'a [u8], end: &'a [u8]) -> Result<Self, TenantDataAccessErrorReason> {
        let begin = TenantDataKey::new(begin)?;
        let end = TenantDataKey::new(end)?;
        if begin.as_bytes() >= end.as_bytes() {
            return Err(TenantDataAccessErrorReason::InvalidRange);
        }
        if begin.as_bytes() < RESERVED_METADATA_END && end.as_bytes() > RESERVED_METADATA_START {
            return Err(TenantDataAccessErrorReason::ReservedRange);
        }
        Ok(Self { begin, end })
    }

    /// Returns the validated inclusive lower bound.
    pub fn begin(&self) -> &[u8] {
        self.begin.as_bytes()
    }

    /// Returns the validated exclusive upper bound.
    pub fn end(&self) -> &[u8] {
        self.end.as_bytes()
    }
}

/// A bounded number of key-value pairs to return from one range read.
#[derive(Clone, Copy)]
pub struct TenantDataRangeLimit(NonZeroUsize);

impl TenantDataRangeLimit {
    /// Largest page accepted by the tenant data capability.
    pub const MAX: usize = MAX_RANGE_RESULTS;

    /// Rejects zero and unbounded result counts.
    pub fn new(value: usize) -> Result<Self, TenantDataAccessErrorReason> {
        let value = NonZeroUsize::new(value)
            .filter(|value| value.get() <= MAX_RANGE_RESULTS)
            .ok_or(TenantDataAccessErrorReason::InvalidRangeLimit)?;
        Ok(Self(value))
    }

    /// Returns the validated maximum result count.
    pub fn get(self) -> usize {
        self.0.get()
    }
}

/// The application view of a tenant transaction.
///
/// This type intentionally exposes no raw transaction handle, commit operation,
/// or option setters. Every key-bearing operation takes a validated data key or
/// range so application callbacks cannot modify kit metadata.
pub struct TenantDataTransaction<'a> {
    inner: &'a Transaction,
    writable: bool,
    mutation_rejected: AtomicBool,
}

impl<'a> TenantDataTransaction<'a> {
    pub(super) fn new(inner: &'a Transaction, writable: bool) -> Self {
        Self {
            inner,
            writable,
            mutation_rejected: AtomicBool::new(false),
        }
    }

    pub(super) fn mutation_rejected(&self) -> bool {
        self.mutation_rejected.load(Ordering::Relaxed)
    }

    fn require_write(&self) -> Result<(), TenantDataAccessErrorReason> {
        if self.writable {
            Ok(())
        } else {
            // Callbacks can ignore a method's Result. The retry adapter must
            // still abort the transaction before it can commit.
            self.mutation_rejected.store(true, Ordering::Relaxed);
            Err(TenantDataAccessErrorReason::KeyChangingMutation)
        }
    }

    /// Applies a validated cluster-side size ceiling before application operations.
    pub fn set_size_limit(&self, limit: TenantTransactionSizeLimit) -> foundationdb::FdbResult<()> {
        self.inner.set_option(TransactionOption::SizeLimit(limit.0))
    }

    /// Reads an application key, preserving FoundationDB's retryable error.
    pub async fn get(
        &self,
        key: TenantDataKey<'_>,
        snapshot: bool,
    ) -> foundationdb::FdbResult<Option<FdbSlice>> {
        self.inner.get(key.as_bytes(), snapshot).await
    }

    /// Writes a bounded value under an application key.
    pub fn set(
        &self,
        key: TenantDataKey<'_>,
        value: &[u8],
    ) -> Result<(), TenantDataAccessErrorReason> {
        self.require_write()?;
        if value.len() > MAX_TENANT_VALUE_BYTES {
            return Err(TenantDataAccessErrorReason::ValueTooLong);
        }
        self.inner.set(key.as_bytes(), value);
        Ok(())
    }

    /// Removes one application key. A read-policy attempt aborts the enclosing
    /// transaction even though this compatibility method returns no result.
    pub fn clear(&self, key: TenantDataKey<'_>) {
        let _ = self.try_clear(key);
    }

    /// Checks the write policy before removing one application key.
    pub fn try_clear(&self, key: TenantDataKey<'_>) -> Result<(), TenantDataAccessErrorReason> {
        self.require_write()?;
        self.inner.clear(key.as_bytes());
        Ok(())
    }

    /// Removes only the validated application interval. A read-policy attempt
    /// aborts the enclosing transaction even though this method returns no result.
    pub fn clear_range(&self, range: &TenantDataRange<'_>) {
        let _ = self.try_clear_range(range);
    }

    /// Checks the write policy before clearing an application interval.
    pub fn try_clear_range(
        &self,
        range: &TenantDataRange<'_>,
    ) -> Result<(), TenantDataAccessErrorReason> {
        self.require_write()?;
        self.inner.clear_range(range.begin(), range.end());
        Ok(())
    }

    /// Applies an atomic mutation to an application key.
    pub fn atomic_op(
        &self,
        key: TenantDataKey<'_>,
        parameter: &[u8],
        mutation: MutationType,
    ) -> Result<(), TenantDataAccessErrorReason> {
        self.require_write()?;
        validate_atomic_mutation(mutation)?;
        if parameter.len() > MAX_TENANT_VALUE_BYTES {
            return Err(TenantDataAccessErrorReason::ValueTooLong);
        }
        self.inner.atomic_op(key.as_bytes(), parameter, mutation);
        Ok(())
    }

    /// Writes a tuple-layer versionstamped key whose stable prefix is tenant data.
    ///
    /// The incomplete versionstamp must lie after the first six key bytes so
    /// commit-time substitution cannot move a key into kit metadata or system space.
    pub fn set_versionstamped_key(
        &self,
        key_template: &[u8],
        value: &[u8],
    ) -> Result<(), TenantDataAccessErrorReason> {
        self.require_write()?;
        validate_versionstamped_key_template(key_template)?;
        if value.len() > MAX_TENANT_VALUE_BYTES {
            return Err(TenantDataAccessErrorReason::ValueTooLong);
        }
        self.inner
            .atomic_op(key_template, value, MutationType::SetVersionstampedKey);
        Ok(())
    }

    /// Reads one bounded page from an application range.
    pub async fn get_range(
        &self,
        range: &TenantDataRange<'_>,
        limit: TenantDataRangeLimit,
        snapshot: bool,
    ) -> foundationdb::FdbResult<FdbValues> {
        self.get_range_with_target_bytes(range, limit, MAX_RANGE_TARGET_BYTES, snapshot)
            .await
    }

    /// Reads a bounded page with an application-selected response byte target.
    pub async fn get_range_with_target(
        &self,
        range: &TenantDataRange<'_>,
        limit: TenantDataRangeLimit,
        target_bytes: TenantDataRangeTargetBytes,
        snapshot: bool,
    ) -> foundationdb::FdbResult<FdbValues> {
        self.get_range_with_target_bytes(range, limit, target_bytes.0.get(), snapshot)
            .await
    }

    async fn get_range_with_target_bytes(
        &self,
        range: &TenantDataRange<'_>,
        limit: TenantDataRangeLimit,
        target_bytes: usize,
        snapshot: bool,
    ) -> foundationdb::FdbResult<FdbValues> {
        let mut options = RangeOption::from((range.begin(), range.end()));
        options.limit = Some(limit.get());
        options.target_bytes = target_bytes;
        self.inner.get_range(&options, 1, snapshot).await
    }
}

fn validate_versionstamped_key_template(
    key_template: &[u8],
) -> Result<(), TenantDataAccessErrorReason> {
    let key_bytes = key_template
        .len()
        .checked_sub(VERSIONSTAMP_OFFSET_BYTES)
        .ok_or(TenantDataAccessErrorReason::KeyChangingMutation)?;
    TenantDataKey::new(&key_template[..key_bytes])?;
    let offset_bytes: [u8; VERSIONSTAMP_OFFSET_BYTES] = key_template[key_bytes..]
        .try_into()
        .map_err(|_| TenantDataAccessErrorReason::KeyChangingMutation)?;
    let offset = usize::try_from(u32::from_le_bytes(offset_bytes))
        .map_err(|_| TenantDataAccessErrorReason::KeyChangingMutation)?;
    let end = offset
        .checked_add(VERSIONSTAMP_BYTES)
        .ok_or(TenantDataAccessErrorReason::KeyChangingMutation)?;
    if offset < MIN_STABLE_KEY_PREFIX_BYTES
        || end > key_bytes
        || key_template[offset..end].iter().any(|byte| *byte != 0xff)
    {
        return Err(TenantDataAccessErrorReason::KeyChangingMutation);
    }
    Ok(())
}

fn validate_atomic_mutation(mutation: MutationType) -> Result<(), TenantDataAccessErrorReason> {
    // Only reviewed value mutations may pass. This upstream enum is non-exhaustive:
    // accepting a future variant by default could let it change a validated key.
    match mutation {
        MutationType::Add
        | MutationType::And
        | MutationType::BitAnd
        | MutationType::Or
        | MutationType::BitOr
        | MutationType::Xor
        | MutationType::BitXor
        | MutationType::AppendIfFits
        | MutationType::Max
        | MutationType::Min
        | MutationType::SetVersionstampedValue
        | MutationType::ByteMin
        | MutationType::ByteMax
        | MutationType::CompareAndClear => Ok(()),
        MutationType::SetVersionstampedKey => Err(TenantDataAccessErrorReason::KeyChangingMutation),
        _ => Err(TenantDataAccessErrorReason::KeyChangingMutation),
    }
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
