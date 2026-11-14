// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Explicit operator-only tenant lifecycle operations.

use std::time::Duration;

use crate::fdb::tenant_name::FoundationDbTenantName;
use foundationdb::tenant::TenantManagement;

use super::TenantHandle;
use super::metadata::{
    read_tenant_metadata, repair_absent_tenant_metadata, repair_empty_tenant_metadata,
    write_tenant_metadata,
};
use super::recovery_handle::{RecoveryTransaction, VerifiedRecoveryTenant};
use crate::fdb::connector::FoundationDbConnector;
use crate::fdb::error::{FdbError, FdbResult, TenantErrorReason};
use crate::keys::{TenantMetadataCreatedAtKey, TenantMetadataSchemaVersionKey, codec::KeyEncoder};

const TENANT_ADMIN_TIMEOUT_MILLIS: i32 = 5_000;
const TENANT_ADMIN_TIMEOUT: Duration = Duration::from_millis(5_000);
const MAX_METADATA_VALUE_BYTES: usize = 64;
// At most two kit keys are allowed. Reading a third detects application data.
const DELETE_SCAN_LIMIT: i32 = 3;
const FDB_TENANT_NOT_EMPTY_CODE: i32 = 2133;
const METADATA_RESTORE_ATTEMPTS: usize = 3;

/// Creates a tenant if absent and validates its versioned metadata.
///
/// Existing tenants are never modified implicitly. If an existing tenant has
/// missing or incompatible metadata, the operation fails for explicit repair.
pub async fn ensure_tenant(
    connector: &FoundationDbConnector,
    tenant: FoundationDbTenantName,
) -> FdbResult<()> {
    let database = connector.database();
    let label = tenant.as_bytes();
    let was_created = match TenantManagement::get_tenant(database, label).await {
        Ok(Some(_info)) => false,
        Ok(None) => {
            TenantManagement::create_tenant(database, label)
                .await
                .map_err(|_| FdbError::Tenant {
                    reason: TenantErrorReason::AdministrationFailed { tenant },
                })?;
            true
        }
        Err(_) => {
            return Err(FdbError::Tenant {
                reason: TenantErrorReason::LookupFailed { tenant },
            });
        }
    };

    let inner = database.open_tenant(label).map_err(|_| FdbError::Tenant {
        reason: TenantErrorReason::OpenFailed { tenant },
    })?;
    let tenant_handle = TenantHandle::new(tenant, inner);

    if was_created {
        return write_tenant_metadata(&tenant_handle, tenant).await;
    }

    read_tenant_metadata(&tenant_handle)
        .await?
        .ensure_compatible(tenant)
}

/// Repairs the crash window between tenant creation and metadata initialization.
///
/// This explicit operator action refuses nonempty tenants and never overwrites
/// existing metadata. A normal `ensure_tenant` call remains fail closed.
pub async fn repair_tenant_metadata(
    connector: &FoundationDbConnector,
    tenant: FoundationDbTenantName,
) -> FdbResult<()> {
    let database = connector.database();
    let inner = database
        .open_tenant(tenant.as_bytes())
        .map_err(|_| FdbError::Tenant {
            reason: TenantErrorReason::OpenFailed { tenant },
        })?;
    let handle = TenantHandle::new(tenant, inner);
    tokio::time::timeout(
        TENANT_ADMIN_TIMEOUT,
        repair_empty_tenant_metadata(&handle, tenant),
    )
    .await
    .map_err(|_| administration_failed(tenant))?
}

/// Operator recovery for an interrupted delete that left both metadata keys
/// absent. Unlike initial provisioning repair, application data may be present.
/// This reinitializes the metadata creation time. The expected tenant ID must
/// come from a trusted record captured before the interrupted deletion.
/// Recovery verifies the resolved ID of the handle used for the write.
pub async fn recover_interrupted_delete(
    connector: &FoundationDbConnector,
    tenant: FoundationDbTenantName,
    expected_tenant_id: i64,
) -> FdbResult<()> {
    tokio::time::timeout(TENANT_ADMIN_TIMEOUT, async {
        let handle = connector
            .recovery_database()
            .open_tenant(tenant)?
            .verify(expected_tenant_id)
            .await?;
        repair_absent_tenant_metadata(&handle, i64::from(TENANT_ADMIN_TIMEOUT_MILLIS)).await
    })
    .await
    .map_err(|_| administration_failed(tenant))?
}

/// Deletes a tenant containing only the kit's own metadata keys.
///
/// FoundationDB checks emptiness before applying a tenant-map deletion, so
/// metadata must be cleared in a committed tenant transaction first. An
/// unconfirmed clear or failed second phase can restore the saved keys only
/// to the original tenant.
pub async fn delete_tenant(
    connector: &FoundationDbConnector,
    tenant: FoundationDbTenantName,
) -> FdbResult<()> {
    let handle = open_delete_tenant(connector, tenant).await?;
    let (transaction, snapshot) = stage_metadata_clear(&handle).await?;
    // The transaction has its own FoundationDB timeout. Do not cancel a
    // possibly in-flight phase-one commit with a Rust future timeout.
    if let Err(error) = transaction.commit().await {
        if !snapshot.entries.is_empty() {
            restore_metadata_with_retries(connector, &snapshot).await?;
        }
        return Err(error);
    }
    finish_delete_after_clear(connector, &snapshot).await
}

async fn open_delete_tenant(
    connector: &FoundationDbConnector,
    tenant: FoundationDbTenantName,
) -> FdbResult<VerifiedRecoveryTenant<'_>> {
    let original_id = tenant_id(connector, tenant)
        .await
        .map_err(|_| administration_failed(tenant))?
        .ok_or(FdbError::Tenant {
            reason: TenantErrorReason::NotProvisioned { tenant },
        })?;
    tokio::time::timeout(
        TENANT_ADMIN_TIMEOUT,
        connector
            .recovery_database()
            .open_tenant(tenant)?
            .verify(original_id),
    )
    .await
    .map_err(|_| administration_failed(tenant))?
}

async fn finish_delete_after_clear(
    connector: &FoundationDbConnector,
    snapshot: &TenantMetadataSnapshot,
) -> FdbResult<()> {
    let tenant = snapshot.tenant;
    let database = connector.database();
    let deletion = tokio::time::timeout(
        TENANT_ADMIN_TIMEOUT,
        TenantManagement::delete_tenant(database, tenant.as_bytes()),
    )
    .await;
    if matches!(deletion, Ok(Ok(()))) {
        return Ok(());
    }

    // A concurrent delete may have completed even if the caller saw an
    // uncertain timeout. Never recreate metadata for a removed tenant.
    let existence = tokio::time::timeout(
        TENANT_ADMIN_TIMEOUT,
        TenantManagement::get_tenant(database, tenant.as_bytes()),
    )
    .await;
    if let Ok(Ok(None)) = existence {
        return Ok(());
    }
    // Even if the lookup is unavailable, attempt to restore before returning.
    // A transient failure must not strand a surviving tenant without metadata.
    restore_metadata_with_retries(connector, snapshot).await?;
    match deletion {
        Ok(Err(error)) if error.code() == FDB_TENANT_NOT_EMPTY_CODE => Err(FdbError::Tenant {
            reason: TenantErrorReason::RepairRequiresEmptyTenant { tenant },
        }),
        _ => Err(administration_failed(tenant)),
    }
}

struct TenantMetadataSnapshot {
    tenant: FoundationDbTenantName,
    original_id: i64,
    entries: Vec<(Vec<u8>, Vec<u8>)>,
}

fn metadata_keys(tenant: FoundationDbTenantName) -> FdbResult<[bytes::Bytes; 2]> {
    Ok([
        TenantMetadataSchemaVersionKey::new()
            .map_err(|_| administration_failed(tenant))?
            .encode_key(),
        TenantMetadataCreatedAtKey::new()
            .map_err(|_| administration_failed(tenant))?
            .encode_key(),
    ])
}

async fn stage_metadata_clear<'handle>(
    handle: &'handle VerifiedRecoveryTenant<'_>,
) -> FdbResult<(RecoveryTransaction<'handle>, TenantMetadataSnapshot)> {
    let tenant = handle.name();
    let mut transaction = handle.create_transaction(i64::from(TENANT_ADMIN_TIMEOUT_MILLIS))?;
    let keys = metadata_keys(tenant)?;
    let existing = transaction
        .read_range(b"", b"\xff", DELETE_SCAN_LIMIT)
        .await?;
    if existing.more() || existing.len() > keys.len() {
        return Err(FdbError::Tenant {
            reason: TenantErrorReason::RepairRequiresEmptyTenant { tenant },
        });
    }
    let mut entries = Vec::with_capacity(keys.len());
    for entry in &existing {
        if entry.key() != keys[0].as_ref() && entry.key() != keys[1].as_ref() {
            return Err(FdbError::Tenant {
                reason: TenantErrorReason::RepairRequiresEmptyTenant { tenant },
            });
        }
        if entry.value().len() > MAX_METADATA_VALUE_BYTES {
            return Err(administration_failed(tenant));
        }
        entries.push((entry.key().to_vec(), entry.value().to_vec()));
    }
    for entry in &existing {
        transaction.clear(entry.key())?;
    }
    // Capture the identity of the handle that read and staged the clear,
    // so restoration can target the same tenant even after an uncertain commit.
    let snapshot = TenantMetadataSnapshot {
        tenant,
        original_id: handle.id(),
        entries,
    };
    Ok((transaction, snapshot))
}

async fn restore_metadata_with_retries(
    connector: &FoundationDbConnector,
    snapshot: &TenantMetadataSnapshot,
) -> FdbResult<()> {
    for _ in 0..METADATA_RESTORE_ATTEMPTS {
        if let Ok(Ok(())) =
            tokio::time::timeout(TENANT_ADMIN_TIMEOUT, restore_metadata(connector, snapshot)).await
        {
            return Ok(());
        }
    }
    Err(administration_failed(snapshot.tenant))
}

async fn restore_metadata(
    connector: &FoundationDbConnector,
    snapshot: &TenantMetadataSnapshot,
) -> FdbResult<()> {
    // A retry opens a fresh handle. The name can resolve to a replacement
    // between attempts, so verify every handle before staging any writes.
    let handle = connector
        .recovery_database()
        .open_tenant(snapshot.tenant)?
        .verify(snapshot.original_id)
        .await?;
    let mut transaction = handle.create_transaction(i64::from(TENANT_ADMIN_TIMEOUT_MILLIS))?;
    for (key, value) in &snapshot.entries {
        // Always stage the repair write. An earlier read could observe the
        // old metadata before a pending phase-one clear commits; skipping the
        // write would then allow that clear to strand a surviving tenant.
        transaction.set(key, value)?;
    }
    transaction.commit().await
}

fn administration_failed(tenant: FoundationDbTenantName) -> FdbError {
    FdbError::Tenant {
        reason: TenantErrorReason::AdministrationFailed { tenant },
    }
}

/// Returns whether a tenant is explicitly provisioned.
pub async fn tenant_exists(
    connector: &FoundationDbConnector,
    tenant: FoundationDbTenantName,
) -> FdbResult<bool> {
    let database = connector.database();
    match TenantManagement::get_tenant(database, tenant.as_bytes()).await {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(_) => Err(FdbError::Tenant {
            reason: TenantErrorReason::LookupFailed { tenant },
        }),
    }
}

/// Returns the cluster-assigned ID for a tenant before an operator deletes it.
///
/// Keep a trusted record of this ID for `recover_interrupted_delete`; tenant
/// names alone cannot distinguish the intended tenant from a later replacement.
pub async fn tenant_id(
    connector: &FoundationDbConnector,
    tenant: FoundationDbTenantName,
) -> FdbResult<Option<i64>> {
    let lookup = tokio::time::timeout(
        TENANT_ADMIN_TIMEOUT,
        TenantManagement::get_tenant(connector.database(), tenant.as_bytes()),
    )
    .await;
    match lookup {
        Ok(Ok(Some(Ok(info)))) => Ok(Some(info.id)),
        Ok(Ok(None)) => Ok(None),
        Ok(Ok(Some(Err(_)))) | Ok(Err(_)) | Err(_) => Err(FdbError::Tenant {
            reason: TenantErrorReason::LookupFailed { tenant },
        }),
    }
}

#[cfg(test)]
#[path = "admin_tests.rs"]
mod tests;
