// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Application key access inside a FoundationDB tenant.

use std::num::NonZeroUsize;

use foundationdb::{
    RangeOption, Transaction,
    future::{FdbSlice, FdbValues},
    options::MutationType,
};
use thiserror::Error;

const RESERVED_METADATA_START: &[u8] = b"__meta";
const RESERVED_METADATA_END: &[u8] = b"__metb";
const MAX_TENANT_KEY_BYTES: usize = 10_000;
const MAX_TENANT_VALUE_BYTES: usize = 100_000;
const MAX_RANGE_RESULTS: usize = 64;
const MAX_RANGE_TARGET_BYTES: usize = 1_000_000;

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
    /// Range reads must have a bounded result count.
    #[error("tenant data range limit is invalid")]
    InvalidRangeLimit,
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
}

impl<'a> TenantDataTransaction<'a> {
    pub(super) const fn new(inner: &'a Transaction) -> Self {
        Self { inner }
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
        if value.len() > MAX_TENANT_VALUE_BYTES {
            return Err(TenantDataAccessErrorReason::ValueTooLong);
        }
        self.inner.set(key.as_bytes(), value);
        Ok(())
    }

    /// Removes one application key.
    pub fn clear(&self, key: TenantDataKey<'_>) {
        self.inner.clear(key.as_bytes());
    }

    /// Removes only the validated application interval.
    pub fn clear_range(&self, range: &TenantDataRange<'_>) {
        self.inner.clear_range(range.begin(), range.end());
    }

    /// Applies an atomic mutation to an application key.
    pub fn atomic_op(
        &self,
        key: TenantDataKey<'_>,
        parameter: &[u8],
        mutation: MutationType,
    ) -> Result<(), TenantDataAccessErrorReason> {
        if parameter.len() > MAX_TENANT_VALUE_BYTES {
            return Err(TenantDataAccessErrorReason::ValueTooLong);
        }
        self.inner.atomic_op(key.as_bytes(), parameter, mutation);
        Ok(())
    }

    /// Reads one bounded page from an application range.
    pub async fn get_range(
        &self,
        range: &TenantDataRange<'_>,
        limit: TenantDataRangeLimit,
        snapshot: bool,
    ) -> foundationdb::FdbResult<FdbValues> {
        let mut options = RangeOption::from((range.begin(), range.end()));
        options.limit = Some(limit.get());
        options.target_bytes = MAX_RANGE_TARGET_BYTES;
        self.inner.get_range(&options, 1, snapshot).await
    }
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
