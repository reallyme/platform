// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use foundationdb::{RangeOption, Transaction, future::FdbValues, options::TransactionOption};
use serde::Deserialize;

use super::{AttemptResult, ReservationErrorReason, ReservationIntent};
use crate::FoundationDbTenantName;
use crate::fdb::tenant::{
    admin::{DELETE_SCAN_LIMIT, MAX_METADATA_VALUE_BYTES, metadata_keys, tenant_map_key},
    data::{RESERVED_METADATA_END, RESERVED_METADATA_START},
    metadata::{RECONCILE_METADATA_SCAN_LIMIT, reconciliation_metadata},
};

#[derive(Deserialize)]
struct TenantMapEntry {
    id: i64,
    lock_state: TenantLockState,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum TenantLockState {
    Unlocked,
    ReadOnly,
    Locked,
}

pub(super) async fn mapping_matches(
    transaction: &Transaction,
    tenant: FoundationDbTenantName,
    expected: i64,
) -> AttemptResult<bool> {
    let key = tenant_map_key(tenant).map_err(|_| ReservationErrorReason::InvalidRecord)?;
    let Some(value) = transaction.get(&key, false).await? else {
        return Ok(false);
    };
    let entry: TenantMapEntry = serde_json::from_slice(value.as_ref())
        .map_err(|_| ReservationErrorReason::InvalidRecord)?;
    if entry.id < 0 {
        return Err(ReservationErrorReason::InvalidRecord.into());
    }
    if entry.id != expected {
        return Ok(false);
    }
    // RAW_ACCESS transactions have no tenant context for the client's lock
    // check. A non-snapshot map read also conflicts with a later lock change.
    if entry.lock_state != TenantLockState::Unlocked {
        return Err(ReservationErrorReason::TenantLocked.into());
    }
    Ok(true)
}

pub(super) async fn stage_clear(
    transaction: &Transaction,
    tenant: FoundationDbTenantName,
    id: i64,
) -> AttemptResult<()> {
    let keys = prefixed_metadata_keys(tenant, id)?;
    let entries = read_range(transaction, id, b"", b"\xff", DELETE_SCAN_LIMIT).await?;
    if entries.more()
        || entries.len() > keys.len()
        || entries.iter().any(|entry| {
            entry.value().len() > MAX_METADATA_VALUE_BYTES
                || !keys.iter().any(|key| entry.key() == key)
        })
    {
        return Err(ReservationErrorReason::InvalidMetadata.into());
    }
    for entry in &entries {
        transaction.clear(entry.key());
    }
    Ok(())
}

pub(super) async fn stage_delete(
    transaction: &Transaction,
    intent: ReservationIntent,
) -> AttemptResult<()> {
    transaction.set_option(TransactionOption::SpecialKeySpaceEnableWrites)?;
    if mapping_matches(transaction, intent.tenant, intent.original_id).await? {
        let key =
            tenant_map_key(intent.tenant).map_err(|_| ReservationErrorReason::InvalidRecord)?;
        transaction.clear(&key);
    }
    Ok(())
}

pub(super) async fn stage_repair(
    transaction: &Transaction,
    tenant: FoundationDbTenantName,
    id: i64,
) -> AttemptResult<()> {
    let [schema, created_at] = prefixed_metadata_keys(tenant, id)?;
    let entries = read_range(
        transaction,
        id,
        RESERVED_METADATA_START,
        RESERVED_METADATA_END,
        RECONCILE_METADATA_SCAN_LIMIT,
    )
    .await?;
    let (version, timestamp) = reconciliation_metadata(tenant, &entries, &schema, &created_at)
        .map_err(|_| ReservationErrorReason::InvalidMetadata)?;
    // Even matching metadata must generate a write conflict with an older
    // clear. The caller commits these sets and the outcome in one transaction.
    transaction.set(&schema, &version);
    transaction.set(&created_at, &timestamp);
    Ok(())
}

fn prefixed_metadata_keys(tenant: FoundationDbTenantName, id: i64) -> AttemptResult<[Vec<u8>; 2]> {
    let prefix = id.to_be_bytes();
    let keys = metadata_keys(tenant).map_err(|_| ReservationErrorReason::InvalidMetadata)?;
    Ok(keys.map(|key| [prefix.as_slice(), key.as_ref()].concat()))
}

async fn read_range(
    transaction: &Transaction,
    id: i64,
    begin: &[u8],
    end: &[u8],
    limit: i32,
) -> AttemptResult<FdbValues> {
    // The verified nonnegative ID is the eight-byte FoundationDB 7.3 tenant
    // prefix. Both bounds remain inside the same tenant keyspace.
    let prefix = id.to_be_bytes();
    let begin = [prefix.as_slice(), begin].concat();
    let end = [prefix.as_slice(), end].concat();
    let mut range = RangeOption::from((begin, end));
    range.limit = Some(usize::try_from(limit).map_err(|_| ReservationErrorReason::InvalidRecord)?);
    Ok(transaction.get_range(&range, 1, false).await?)
}
