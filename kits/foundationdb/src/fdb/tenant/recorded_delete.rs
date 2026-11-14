// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use foundationdb::options::TransactionOption;

use super::{TENANT_ADMIN_TIMEOUT, TENANT_ADMIN_TIMEOUT_MILLIS, administration_failed, tenant_id};
use crate::fdb::{
    connector::FoundationDbConnector,
    error::{FdbError, FdbResult, FdbSetupErrorReason},
    tenant::metadata::confirm_or_repair_tenant_metadata,
    tenant_name::FoundationDbTenantName,
};

// The native API 730 client exposes the UID as an immutable special key.
const CLUSTER_ID_KEY: &[u8] = b"\xff\xff/cluster_id";

/// The FoundationDB cluster UID used to scope a durable delete record.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TenantAdminClusterId([u8; 32]);

impl TenantAdminClusterId {
    /// Parses the native client's 32-character hexadecimal UID representation.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let uid: [u8; 32] = bytes.try_into().ok()?;
        uid.iter()
            .all(u8::is_ascii_hexdigit)
            .then(|| Self(uid.map(|byte| byte.to_ascii_lowercase())))
    }

    /// Returns the canonical UID bytes for the operator record.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Reads the cluster UID from FoundationDB for durable operator records.
pub async fn tenant_admin_cluster_id(
    connector: &FoundationDbConnector,
) -> FdbResult<TenantAdminClusterId> {
    let failed = FdbError::Setup {
        reason: FdbSetupErrorReason::ClusterIdentityUnavailable,
    };
    tokio::time::timeout(TENANT_ADMIN_TIMEOUT, async {
        let transaction = connector.database().create_trx().map_err(|_| failed)?;
        transaction
            .set_option(TransactionOption::Timeout(TENANT_ADMIN_TIMEOUT_MILLIS))
            .map_err(|_| failed)?;
        let value = transaction
            .get(CLUSTER_ID_KEY, false)
            .await
            .map_err(|_| failed)?
            .ok_or(failed)?;
        TenantAdminClusterId::from_bytes(value.as_ref()).ok_or(failed)
    })
    .await
    .map_err(|_| failed)?
}

/// The outcome of reconciling a durable intent against its original tenant ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantDeleteResolution {
    /// The recorded ID is absent from the tenant name's current mapping.
    OriginalAbsent,
    /// A transaction confirmed or repaired the original tenant's metadata.
    MetadataSafe,
}

/// Reconciles the original mapping or confirms metadata on the recorded tenant.
///
/// The caller must validate the record's cluster identity and serialize this
/// operation against any worker still initiating that record's delete.
pub async fn reconcile_recorded_delete(
    connector: &FoundationDbConnector,
    tenant: FoundationDbTenantName,
    original_id: i64,
) -> FdbResult<TenantDeleteResolution> {
    if tenant_id(connector, tenant).await? != Some(original_id) {
        return Ok(TenantDeleteResolution::OriginalAbsent);
    }
    tokio::time::timeout(TENANT_ADMIN_TIMEOUT, async {
        let handle = connector
            .recovery_database()
            .open_tenant(tenant)?
            .verify(original_id)
            .await?;
        confirm_or_repair_tenant_metadata(&handle, i64::from(TENANT_ADMIN_TIMEOUT_MILLIS)).await?;
        Ok(TenantDeleteResolution::MetadataSafe)
    })
    .await
    .map_err(|_| administration_failed(tenant))?
}
