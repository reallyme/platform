// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Explicit operator-only tenant lifecycle operations.

use std::time::Duration;

use crate::fdb::tenant_name::FoundationDbTenantName;
use foundationdb::{Database, RangeOption, tenant::TenantManagement};

use super::TenantHandle;
use super::metadata::{read_tenant_metadata, repair_empty_tenant_metadata, write_tenant_metadata};
use crate::fdb::connector::FoundationDbConnector;
use crate::fdb::error::{FdbError, FdbResult, TenantErrorReason};
use crate::keys::{TenantMetadataCreatedAtKey, TenantMetadataSchemaVersionKey, codec::KeyEncoder};

const TENANT_ADMIN_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_METADATA_VALUE_BYTES: usize = 64;
const FDB_TENANT_NOT_EMPTY_CODE: i32 = 2133;

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

/// Deletes a tenant containing only the kit's own metadata keys.
///
/// FoundationDB checks emptiness before applying a tenant-map deletion, so
/// metadata must be cleared in a committed tenant transaction first. A failed
/// second phase restores those keys when the tenant still exists.
pub async fn delete_tenant(
    connector: &FoundationDbConnector,
    tenant: FoundationDbTenantName,
) -> FdbResult<()> {
    let database = connector.database();
    let metadata = tokio::time::timeout(
        TENANT_ADMIN_TIMEOUT,
        clear_metadata_if_empty(database, tenant),
    )
    .await
    .map_err(|_| administration_failed(tenant))??;
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
    .await
    .map_err(|_| administration_failed(tenant))?;
    match existence {
        Ok(None) => return Ok(()),
        Ok(Some(_)) => {}
        Err(_) => return Err(administration_failed(tenant)),
    }
    tokio::time::timeout(
        TENANT_ADMIN_TIMEOUT,
        restore_metadata(database, tenant, &metadata),
    )
    .await
    .map_err(|_| administration_failed(tenant))??;
    match deletion {
        Ok(Err(error)) if error.code() == FDB_TENANT_NOT_EMPTY_CODE => Err(FdbError::Tenant {
            reason: TenantErrorReason::RepairRequiresEmptyTenant { tenant },
        }),
        _ => Err(administration_failed(tenant)),
    }
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

async fn clear_metadata_if_empty(
    database: &Database,
    tenant: FoundationDbTenantName,
) -> FdbResult<Vec<(Vec<u8>, Vec<u8>)>> {
    if TenantManagement::get_tenant(database, tenant.as_bytes())
        .await
        .map_err(|_| administration_failed(tenant))?
        .is_none()
    {
        return Err(FdbError::Tenant {
            reason: TenantErrorReason::NotProvisioned { tenant },
        });
    }
    let handle = database
        .open_tenant(tenant.as_bytes())
        .map_err(|_| administration_failed(tenant))?;
    let transaction = handle
        .create_trx()
        .map_err(|_| administration_failed(tenant))?;
    let keys = metadata_keys(tenant)?;
    let mut range = RangeOption::from((b"".as_slice(), b"\xff".as_slice()));
    range.limit = Some(3);
    let existing = transaction
        .get_range(&range, 1, false)
        .await
        .map_err(|_| administration_failed(tenant))?;
    let mut metadata = Vec::with_capacity(2);
    for entry in &existing {
        if entry.key() != keys[0].as_ref() && entry.key() != keys[1].as_ref() {
            return Err(FdbError::Tenant {
                reason: TenantErrorReason::RepairRequiresEmptyTenant { tenant },
            });
        }
        if entry.value().len() > MAX_METADATA_VALUE_BYTES {
            return Err(administration_failed(tenant));
        }
        metadata.push((entry.key().to_vec(), entry.value().to_vec()));
    }
    for entry in &existing {
        transaction.clear(entry.key());
    }
    transaction
        .commit()
        .await
        .map_err(|_| administration_failed(tenant))?;
    Ok(metadata)
}

async fn restore_metadata(
    database: &Database,
    tenant: FoundationDbTenantName,
    metadata: &[(Vec<u8>, Vec<u8>)],
) -> FdbResult<()> {
    let handle = database
        .open_tenant(tenant.as_bytes())
        .map_err(|_| administration_failed(tenant))?;
    let transaction = handle
        .create_trx()
        .map_err(|_| administration_failed(tenant))?;
    for (key, value) in metadata {
        if transaction
            .get(key, false)
            .await
            .map_err(|_| administration_failed(tenant))?
            .is_none()
        {
            transaction.set(key, value);
        }
    }
    transaction
        .commit()
        .await
        .map(|_| ())
        .map_err(|_| administration_failed(tenant))
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

#[cfg(test)]
#[path = "admin_tests.rs"]
mod tests;
