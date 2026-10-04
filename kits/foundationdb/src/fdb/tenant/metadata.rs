// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Versioned metadata stored inside each FoundationDB tenant.

use std::sync::Arc;
#[cfg(feature = "tenant-admin")]
use std::time::{SystemTime, UNIX_EPOCH};

use crate::fdb::tenant_name::FoundationDbTenantName;
use bytes::Bytes;
use foundationdb::FdbError as FdbBindingError;

use super::TenantHandle;
use crate::fdb::error::{
    FdbError, FdbQueryErrorReason, FdbResult, TenantErrorReason, TenantMetadataField,
};
#[cfg(feature = "tenant-admin")]
use crate::fdb::transaction::WriteTxnPolicy;
use crate::fdb::transaction::{ReadTxnPolicy, TenantTransactionPolicy};
use crate::keys::{TenantMetadataCreatedAtKey, TenantMetadataSchemaVersionKey, codec::KeyEncoder};

const TENANT_METADATA_SCHEMA_KEY_LENGTH: usize = 4;
const TENANT_METADATA_CREATED_AT_LENGTH: usize = 8;
const CURRENT_TENANT_SCHEMA_VERSION: u32 = 1;

pub(super) struct TenantMetadataRead {
    schema_version: u32,
    created_at: u64,
}

impl TenantMetadataRead {
    pub(super) fn ensure_compatible(&self, tenant: FoundationDbTenantName) -> FdbResult<()> {
        if self.schema_version != CURRENT_TENANT_SCHEMA_VERSION {
            return Err(FdbError::Tenant {
                reason: TenantErrorReason::SchemaVersionUnsupported {
                    tenant,
                    schema_version: self.schema_version,
                },
            });
        }

        if self.created_at == 0 {
            return Err(FdbError::Tenant {
                reason: TenantErrorReason::MetadataMalformed {
                    tenant,
                    field: TenantMetadataField::CreatedAt,
                },
            });
        }

        Ok(())
    }
}

#[derive(Clone)]
struct TenantMetadataKeys {
    schema_version_key: Bytes,
    created_at_key: Bytes,
}

impl TenantMetadataKeys {
    fn current() -> FdbResult<Self> {
        let schema_version_key =
            TenantMetadataSchemaVersionKey::new().map_err(|_| FdbError::Query {
                reason: FdbQueryErrorReason::MetadataKeyInvalid,
            })?;
        let created_at_key = TenantMetadataCreatedAtKey::new().map_err(|_| FdbError::Query {
            reason: FdbQueryErrorReason::MetadataKeyInvalid,
        })?;

        Ok(Self {
            schema_version_key: schema_version_key.encode_key(),
            created_at_key: created_at_key.encode_key(),
        })
    }
}

pub(super) async fn read_tenant_metadata(
    tenant_handle: &TenantHandle,
) -> FdbResult<TenantMetadataRead> {
    let keys = TenantMetadataKeys::current()?;
    let tenant = tenant_handle.tenant();
    let (schema_raw, created_at_raw) = tenant_handle
        .transact_boxed_arc_raw(
            Arc::new(keys),
            |trx, metadata| {
                Box::pin(async move {
                    // Preserve the binding error so FoundationDB can apply
                    // its actual retryability rules to this transaction.
                    let schema_raw = trx.get(&metadata.schema_version_key, false).await?;
                    let created_at_raw = trx.get(&metadata.created_at_key, false).await?;
                    Ok((schema_raw, created_at_raw))
                })
            },
            TenantTransactionPolicy::Read(ReadTxnPolicy::default()),
        )
        .await
        .map_err(|_error: FdbBindingError| FdbError::Query {
            reason: FdbQueryErrorReason::TransactionFailed,
        })?;

    let schema = schema_raw.ok_or(FdbError::Tenant {
        reason: TenantErrorReason::MetadataMissing {
            tenant,
            field: TenantMetadataField::SchemaVersion,
        },
    })?;
    let created_at = created_at_raw.ok_or(FdbError::Tenant {
        reason: TenantErrorReason::MetadataMissing {
            tenant,
            field: TenantMetadataField::CreatedAt,
        },
    })?;

    Ok(TenantMetadataRead {
        schema_version: parse_schema_version(tenant, schema.as_ref())?,
        created_at: parse_created_at(tenant, created_at.as_ref())?,
    })
}

#[cfg(feature = "tenant-admin")]
pub(super) async fn write_tenant_metadata(
    tenant_handle: &TenantHandle,
    tenant: FoundationDbTenantName,
) -> FdbResult<()> {
    let (schema_version, created_at) = tenant_metadata_now(tenant)?;
    let keys = Arc::new(TenantMetadataKeys::current()?);

    tenant_handle
        .transact_boxed_arc_raw(
            keys,
            |trx, keys| {
                Box::pin(async move {
                    trx.set(
                        &keys.schema_version_key,
                        schema_version.to_le_bytes().as_ref(),
                    );
                    trx.set(&keys.created_at_key, created_at.to_le_bytes().as_ref());
                    Ok(())
                })
            },
            TenantTransactionPolicy::Write(WriteTxnPolicy::default()),
        )
        .await
        .map_err(|_error: FdbBindingError| FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        })
}

fn parse_schema_version(tenant: FoundationDbTenantName, raw: &[u8]) -> FdbResult<u32> {
    if raw.len() != TENANT_METADATA_SCHEMA_KEY_LENGTH {
        return Err(FdbError::Tenant {
            reason: TenantErrorReason::MetadataMalformed {
                tenant,
                field: TenantMetadataField::SchemaVersion,
            },
        });
    }
    let mut version_bytes = [0_u8; TENANT_METADATA_SCHEMA_KEY_LENGTH];
    version_bytes.copy_from_slice(raw);
    let parsed = u32::from_le_bytes(version_bytes);
    if parsed != CURRENT_TENANT_SCHEMA_VERSION {
        return Err(FdbError::Tenant {
            reason: TenantErrorReason::SchemaVersionUnsupported {
                tenant,
                schema_version: parsed,
            },
        });
    }
    Ok(parsed)
}

fn parse_created_at(tenant: FoundationDbTenantName, raw: &[u8]) -> FdbResult<u64> {
    if raw.len() != TENANT_METADATA_CREATED_AT_LENGTH {
        return Err(FdbError::Tenant {
            reason: TenantErrorReason::MetadataMalformed {
                tenant,
                field: TenantMetadataField::CreatedAt,
            },
        });
    }
    let mut created_at_bytes = [0_u8; TENANT_METADATA_CREATED_AT_LENGTH];
    created_at_bytes.copy_from_slice(raw);
    Ok(u64::from_le_bytes(created_at_bytes))
}

#[cfg(feature = "tenant-admin")]
fn tenant_metadata_now(tenant: FoundationDbTenantName) -> FdbResult<(u32, u64)> {
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| FdbError::Tenant {
            reason: TenantErrorReason::MetadataMalformed {
                tenant,
                field: TenantMetadataField::CreatedAt,
            },
        })?
        .as_secs();
    Ok((CURRENT_TENANT_SCHEMA_VERSION, created_at))
}

#[cfg(feature = "tenant-admin")]
pub(super) async fn repair_empty_tenant_metadata(
    tenant_handle: &TenantHandle,
    tenant: FoundationDbTenantName,
) -> FdbResult<()> {
    let keys = TenantMetadataKeys::current()?;
    let (schema_version, created_at) = tenant_metadata_now(tenant)?;
    let transaction = tenant_handle
        .inner()
        .create_trx()
        .map_err(|_| FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        })?;
    let mut range = foundationdb::RangeOption::from((b"".as_slice(), b"\xff".as_slice()));
    range.limit = Some(1);
    let existing = transaction
        .get_range(&range, 1, false)
        .await
        .map_err(|_| FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        })?;
    if !existing.is_empty() {
        return Err(FdbError::Tenant {
            reason: TenantErrorReason::RepairRequiresEmptyTenant { tenant },
        });
    }
    // This is one transaction: a racing writer conflicts with the empty-range
    // read, so repair cannot silently claim a tenant containing app data.
    transaction.set(
        &keys.schema_version_key,
        schema_version.to_le_bytes().as_ref(),
    );
    transaction.set(&keys.created_at_key, created_at.to_le_bytes().as_ref());
    transaction
        .commit()
        .await
        .map(|_| ())
        .map_err(|_| FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        })
}

/// Explicitly recovers a tenant whose two metadata keys were both lost during
/// an interrupted admin deletion. Application data may still be present.
#[cfg(feature = "tenant-admin")]
pub(super) async fn repair_absent_tenant_metadata(
    tenant_handle: &TenantHandle,
    tenant: FoundationDbTenantName,
) -> FdbResult<()> {
    let keys = TenantMetadataKeys::current()?;
    let (schema_version, created_at) = tenant_metadata_now(tenant)?;
    let transaction = tenant_handle
        .inner()
        .create_trx()
        .map_err(|_| FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        })?;
    // No metadata key may exist, including keys from a foreign application or
    // a newer kit schema. The range read conflicts with concurrent writes.
    let mut metadata_range =
        foundationdb::RangeOption::from((b"__meta".as_slice(), b"__metb".as_slice()));
    metadata_range.limit = Some(1);
    let existing = transaction
        .get_range(&metadata_range, 1, false)
        .await
        .map_err(|_| FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        })?;
    if !existing.is_empty() {
        return Err(FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        });
    }
    transaction.set(
        &keys.schema_version_key,
        schema_version.to_le_bytes().as_ref(),
    );
    transaction.set(&keys.created_at_key, created_at.to_le_bytes().as_ref());
    transaction
        .commit()
        .await
        .map(|_| ())
        .map_err(|_| FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        })
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
