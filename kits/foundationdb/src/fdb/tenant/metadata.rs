// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Versioned metadata stored inside each FoundationDB tenant.

use std::sync::Arc;
#[cfg(feature = "tenant-admin")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "tenant-admin")]
use zeroize::Zeroizing;

use crate::fdb::tenant_name::FoundationDbTenantName;
use crate::generated::reallyme::platform::storage::v1::{CreationTime, SchemaVersion};
use crate::persisted_record::decode_record;
#[cfg(feature = "tenant-admin")]
use crate::persisted_record::encode_record;
use bytes::Bytes;
use foundationdb::FdbError as FdbBindingError;
#[cfg(feature = "tenant-admin")]
use foundationdb::future::FdbValues;

use super::TenantHandle;
#[cfg(feature = "tenant-admin")]
use super::data::{RESERVED_METADATA_END, RESERVED_METADATA_START};
#[cfg(feature = "tenant-admin")]
use super::recovery_handle::VerifiedRecoveryTenant;
use crate::fdb::error::{
    FdbError, FdbQueryErrorReason, FdbResult, TenantErrorReason, TenantMetadataField,
};
#[cfg(feature = "tenant-admin")]
use crate::fdb::transaction::WriteTxnPolicy;
use crate::fdb::transaction::{ReadTxnPolicy, TenantTransactionPolicy};
use crate::keys::{TenantMetadataCreatedAtKey, TenantMetadataSchemaVersionKey, codec::KeyEncoder};

#[cfg(feature = "tenant-admin")]
type EncodedTenantMetadata = (Zeroizing<Vec<u8>>, Zeroizing<Vec<u8>>);
const CURRENT_TENANT_SCHEMA_VERSION: u32 = 1;
#[cfg(feature = "tenant-admin")]
pub(super) const RECONCILE_METADATA_SCAN_LIMIT: i32 = 3;

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
                    let (schema, created) = encode_metadata(schema_version, created_at);
                    trx.set(&keys.schema_version_key, &schema);
                    trx.set(&keys.created_at_key, &created);
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
    let parsed = if let Ok(legacy) = raw.try_into() {
        u32::from_le_bytes(legacy)
    } else {
        let message: SchemaVersion = decode_record(raw)
            .map_err(|_| malformed_metadata(tenant, TenantMetadataField::SchemaVersion))?;
        message.version
    };
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
    if let Ok(legacy) = raw.try_into() {
        return Ok(u64::from_le_bytes(legacy));
    }
    let message: CreationTime = decode_record(raw)
        .map_err(|_| malformed_metadata(tenant, TenantMetadataField::CreatedAt))?;
    Ok(message.unix_seconds)
}

fn malformed_metadata(tenant: FoundationDbTenantName, field: TenantMetadataField) -> FdbError {
    FdbError::Tenant {
        reason: TenantErrorReason::MetadataMalformed { tenant, field },
    }
}

#[cfg(feature = "tenant-admin")]
fn encode_metadata(schema_version: u32, created_at: u64) -> EncodedTenantMetadata {
    let schema = SchemaVersion::default().with_version(schema_version);
    let created = CreationTime::default().with_unix_seconds(created_at);
    (encode_record(&schema), encode_record(&created))
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
    let (schema, created) = encode_metadata(schema_version, created_at);
    transaction.set(&keys.schema_version_key, &schema);
    transaction.set(&keys.created_at_key, &created);
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
    tenant_handle: &VerifiedRecoveryTenant<'_>,
    timeout_millis: i64,
) -> FdbResult<()> {
    let tenant = tenant_handle.name();
    let keys = TenantMetadataKeys::current()?;
    let (schema_version, created_at) = tenant_metadata_now(tenant)?;
    let mut transaction = tenant_handle.create_transaction(timeout_millis)?;
    // No metadata key may exist, including keys from a foreign application or
    // a newer kit schema. The range read conflicts with concurrent writes.
    if !transaction
        .range_is_empty(RESERVED_METADATA_START, RESERVED_METADATA_END)
        .await?
    {
        return Err(FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { tenant },
        });
    }
    let (schema, created) = encode_metadata(schema_version, created_at);
    transaction.set(&keys.schema_version_key, &schema)?;
    transaction.set(&keys.created_at_key, &created)?;
    transaction.commit().await
}

#[cfg(feature = "tenant-admin")]
pub(super) async fn confirm_or_repair_tenant_metadata(
    tenant_handle: &VerifiedRecoveryTenant<'_>,
    timeout_millis: i64,
) -> FdbResult<()> {
    let tenant = tenant_handle.name();
    let keys = TenantMetadataKeys::current()?;
    let mut transaction = tenant_handle.create_transaction(timeout_millis)?;
    let entries = transaction
        .read_range(
            RESERVED_METADATA_START,
            RESERVED_METADATA_END,
            RECONCILE_METADATA_SCAN_LIMIT,
        )
        .await?;
    let (schema, created_at) = reconciliation_metadata(
        tenant,
        &entries,
        &keys.schema_version_key,
        &keys.created_at_key,
    )?;
    // Write even when both values match. The write conflicts with a clear
    // that read the metadata before the operator restarted.
    transaction.set(&keys.schema_version_key, &schema)?;
    transaction.set(&keys.created_at_key, &created_at)?;
    transaction.commit().await
}

#[cfg(feature = "tenant-admin")]
pub(super) fn reconciliation_metadata(
    tenant: FoundationDbTenantName,
    entries: &FdbValues,
    schema_key: &[u8],
    created_at_key: &[u8],
) -> FdbResult<EncodedTenantMetadata> {
    let failed = FdbError::Tenant {
        reason: TenantErrorReason::AdministrationFailed { tenant },
    };
    if entries.more() {
        return Err(failed);
    }
    let (schema_version, created_at) = if entries.is_empty() {
        tenant_metadata_now(tenant)?
    } else {
        // Inspect the entire reserved namespace before confirming any values.
        // Partial metadata or an unknown key requires manual review.
        if entries.len() != 2 {
            return Err(failed);
        }
        let schema = entries
            .iter()
            .find(|entry| entry.key() == schema_key)
            .ok_or(failed)?;
        let created = entries
            .iter()
            .find(|entry| entry.key() == created_at_key)
            .ok_or(failed)?;
        let metadata = TenantMetadataRead {
            schema_version: parse_schema_version(tenant, schema.value())?,
            created_at: parse_created_at(tenant, created.value())?,
        };
        metadata.ensure_compatible(tenant)?;
        (metadata.schema_version, metadata.created_at)
    };
    Ok(encode_metadata(schema_version, created_at))
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
