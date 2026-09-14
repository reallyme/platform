// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Explicit operator-only tenant lifecycle operations.

use crate::fdb::tenant_name::FoundationDbTenantName;
use foundationdb::{Database, tenant::TenantManagement};

use super::TenantHandle;
use super::metadata::{read_tenant_metadata, write_tenant_metadata};
use crate::fdb::error::{FdbError, FdbResult, TenantErrorReason};

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

/// Deletes a tenant, failing if FoundationDB rejects the lifecycle operation.
///
/// FoundationDB tenant deletion requires the tenant to be empty; this helper
/// deliberately does not clear tenant data or weaken that safety boundary.
pub async fn delete_tenant(database: &Database, tenant: FoundationDbTenantName) -> FdbResult<()> {
    TenantManagement::delete_tenant(database, tenant.as_bytes())
        .await
        .map_err(|_| FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        })
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
