// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Explicit operator-only tenant lifecycle operations.

use std::time::Duration;

use crate::fdb::tenant_name::FoundationDbTenantName;
use foundationdb::{Database, options::TransactionOption, tenant::TenantManagement};
use reallyme_codec::base64::base64_to_bytes;
use serde::Deserialize;

use super::TenantHandle;
use super::metadata::{read_tenant_metadata, repair_empty_tenant_metadata, write_tenant_metadata};
use crate::fdb::error::{FdbError, FdbResult, TenantErrorReason};
use crate::keys::{TenantMetadataCreatedAtKey, TenantMetadataSchemaVersionKey, codec::KeyEncoder};

const TENANT_MAP_PREFIX: &[u8] = b"\xff\xff/management/tenant/map/";
const TENANT_ADMIN_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_TENANT_MAP_VALUE_BYTES: usize = 1_024;
const MAX_TENANT_PREFIX_BYTES: usize = 64;

#[derive(Deserialize)]
struct TenantMapRecord {
    prefix: TenantPrefixRecord,
}

#[derive(Deserialize)]
struct TenantPrefixRecord {
    base64: String,
}

/// Creates a tenant if absent and validates its versioned metadata.
///
/// Existing tenants are never modified implicitly. If an existing tenant has
/// missing or incompatible metadata, the operation fails for explicit repair.
pub async fn ensure_tenant(database: &Database, tenant: FoundationDbTenantName) -> FdbResult<()> {
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
    database: &Database,
    tenant: FoundationDbTenantName,
) -> FdbResult<()> {
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

/// Deletes an empty tenant, including only the kit's two metadata keys.
///
/// Metadata clearing and tenant deletion share one FoundationDB transaction.
/// If any app data remains, FoundationDB rejects the delete and retains the
/// metadata. Operators can query `tenant_exists` after an uncertain timeout.
pub async fn delete_tenant(database: &Database, tenant: FoundationDbTenantName) -> FdbResult<()> {
    tokio::time::timeout(TENANT_ADMIN_TIMEOUT, delete_empty_tenant(database, tenant))
        .await
        .map_err(|_| FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        })?
}

async fn delete_empty_tenant(database: &Database, tenant: FoundationDbTenantName) -> FdbResult<()> {
    let map_key = tenant_map_key(tenant)?;
    let transaction = database
        .create_trx()
        .map_err(|_| administration_failed(tenant))?;
    transaction
        .set_option(TransactionOption::SpecialKeySpaceEnableWrites)
        .map_err(|_| administration_failed(tenant))?;
    let map_value = transaction
        .get(&map_key, false)
        .await
        .map_err(|_| administration_failed(tenant))?
        .ok_or(FdbError::Tenant {
            reason: TenantErrorReason::NotProvisioned { tenant },
        })?;
    let prefix = decode_tenant_prefix(map_value.as_ref(), tenant)?;

    for metadata_key in [
        TenantMetadataSchemaVersionKey::new()
            .map_err(|_| administration_failed(tenant))?
            .encode_key(),
        TenantMetadataCreatedAtKey::new()
            .map_err(|_| administration_failed(tenant))?
            .encode_key(),
    ] {
        let raw_key = prefixed_metadata_key(&prefix, metadata_key.as_ref(), tenant)?;
        transaction.clear(&raw_key);
    }
    transaction.clear(&map_key);
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

fn tenant_map_key(tenant: FoundationDbTenantName) -> FdbResult<Vec<u8>> {
    let length = TENANT_MAP_PREFIX
        .len()
        .checked_add(tenant.as_bytes().len())
        .ok_or_else(|| administration_failed(tenant))?;
    let mut key = Vec::with_capacity(length);
    key.extend_from_slice(TENANT_MAP_PREFIX);
    key.extend_from_slice(tenant.as_bytes());
    Ok(key)
}

fn decode_tenant_prefix(raw: &[u8], tenant: FoundationDbTenantName) -> FdbResult<Vec<u8>> {
    if raw.len() > MAX_TENANT_MAP_VALUE_BYTES {
        return Err(administration_failed(tenant));
    }
    let record: TenantMapRecord =
        serde_json::from_slice(raw).map_err(|_| administration_failed(tenant))?;
    let prefix =
        base64_to_bytes(&record.prefix.base64).map_err(|_| administration_failed(tenant))?;
    if prefix.is_empty() || prefix.len() > MAX_TENANT_PREFIX_BYTES {
        return Err(administration_failed(tenant));
    }
    Ok(prefix)
}

fn prefixed_metadata_key(
    prefix: &[u8],
    metadata_key: &[u8],
    tenant: FoundationDbTenantName,
) -> FdbResult<Vec<u8>> {
    let length = prefix
        .len()
        .checked_add(metadata_key.len())
        .ok_or_else(|| administration_failed(tenant))?;
    let mut key = Vec::with_capacity(length);
    key.extend_from_slice(prefix);
    key.extend_from_slice(metadata_key);
    Ok(key)
}

/// Returns whether a tenant is explicitly provisioned.
pub async fn tenant_exists(database: &Database, tenant: FoundationDbTenantName) -> FdbResult<bool> {
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
