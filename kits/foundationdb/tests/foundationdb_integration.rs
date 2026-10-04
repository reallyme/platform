// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![deny(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use foundationdb::options::MutationType;
#[cfg(feature = "tenant-admin")]
use reallyme_foundationdb_kit::fdb::tenant::admin;
use reallyme_foundationdb_kit::fdb::transaction::{TenantTransactionPolicy, WriteTxnPolicy};
use reallyme_foundationdb_kit::{
    FdbConfig, FdbResult, FoundationDbConnector, FoundationDbTenantName,
    TenantDataAccessErrorReason, TenantDataKey, TenantDataRange, verify_ready,
};
#[cfg(feature = "tenant-admin")]
use reallyme_foundationdb_kit::{FdbError, TenantErrorReason};

const INTEGRATION_TENANT: &str = "foundationdb-kit-integration";

#[tokio::test]
#[ignore = "requires a live FoundationDB cluster"]
async fn live_connector_proves_cluster_and_tenant_readiness() -> FdbResult<()> {
    let config = FdbConfig::from_env()?;
    // SAFETY: the test drops the connector before the test process exits.
    #[allow(unsafe_code)]
    let connector = unsafe { FoundationDbConnector::connect(&config) }?;
    #[allow(
        clippy::expect_used,
        reason = "the fixed integration fixture should fail loudly if its invariant changes"
    )]
    let tenant = FoundationDbTenantName::new(INTEGRATION_TENANT)
        .expect("the fixed integration tenant name must remain valid");
    let report = verify_ready(&connector, &[tenant]).await?;

    assert_eq!(report.api_version, config.fdb_api_version());
    assert!(report.read_version >= 0);
    assert!(
        report
            .main_thread_busyness
            .is_none_or(|busyness| busyness.is_finite() && busyness >= 0.0)
    );

    let handle = connector.open_tenant(tenant).await?;
    handle
        .transact_boxed(
            (),
            |transaction, _| {
                Box::pin(async move {
                    // Check the public application boundary against a live tenant: the
                    // metadata namespace must remain inaccessible even during a write.
                    assert!(matches!(
                        TenantDataKey::new(b"__meta/v1/schema_version"),
                        Err(TenantDataAccessErrorReason::ReservedKey)
                    ));
                    assert!(matches!(
                        TenantDataRange::new(b"__met", b"__metb"),
                        Err(TenantDataAccessErrorReason::ReservedRange)
                    ));
                    let key = TenantDataKey::new(b"kit-integration/data")
                        .expect("integration data key must remain valid");
                    assert!(matches!(
                        transaction.atomic_op(
                            key,
                            b"versionstamp",
                            MutationType::SetVersionstampedKey
                        ),
                        Err(TenantDataAccessErrorReason::KeyChangingMutation)
                    ));
                    transaction
                        .set(key, b"value")
                        .expect("integration value must remain valid");
                    Ok::<(), foundationdb::FdbError>(())
                })
            },
            TenantTransactionPolicy::Write(WriteTxnPolicy::default()),
        )
        .await
        .map_err(|_| reallyme_foundationdb_kit::FdbError::Query {
            reason: reallyme_foundationdb_kit::FdbQueryErrorReason::TransactionFailed,
        })?;

    let stored = handle
        .transact_boxed(
            (),
            |transaction, _| {
                Box::pin(async move {
                    let key = TenantDataKey::new(b"kit-integration/data")
                        .expect("integration data key must remain valid");
                    transaction
                        .get(key, false)
                        .await
                        .map(|value| value.map(|bytes| bytes.as_ref().to_vec()))
                })
            },
            TenantTransactionPolicy::Read(
                reallyme_foundationdb_kit::fdb::transaction::ReadTxnPolicy::default(),
            ),
        )
        .await
        .map_err(|_| reallyme_foundationdb_kit::FdbError::Query {
            reason: reallyme_foundationdb_kit::FdbQueryErrorReason::TransactionFailed,
        })?;
    assert_eq!(stored.as_deref(), Some(b"value".as_slice()));
    // Application data writes must leave the kit's metadata readable.
    verify_ready(&connector, &[tenant]).await?;
    Ok(())
}

#[cfg(feature = "tenant-admin")]
#[tokio::test]
#[ignore = "requires a live FoundationDB cluster"]
async fn live_tenant_delete_clears_metadata_but_refuses_application_data() -> FdbResult<()> {
    let config = FdbConfig::from_env()?;
    // SAFETY: the test drops the connector before the test process exits.
    #[allow(unsafe_code)]
    let connector = unsafe { FoundationDbConnector::connect(&config) }?;
    let tenant = FoundationDbTenantName::new("foundationdb-kit-delete-integration")
        .expect("fixed tenant fixture");
    admin::ensure_tenant(&connector, tenant).await?;
    admin::delete_tenant(&connector, tenant).await?;
    assert!(!admin::tenant_exists(&connector, tenant).await?);

    admin::ensure_tenant(&connector, tenant).await?;
    let handle = connector.open_tenant(tenant).await?;
    handle
        .transact_boxed(
            (),
            |transaction, _| {
                Box::pin(async move {
                    let key =
                        TenantDataKey::new(b"delete-integration/data").expect("fixed data key");
                    transaction.set(key, b"value").expect("valid value");
                    Ok::<(), foundationdb::FdbError>(())
                })
            },
            TenantTransactionPolicy::Write(WriteTxnPolicy::default()),
        )
        .await
        .map_err(|_| FdbError::Query {
            reason: reallyme_foundationdb_kit::FdbQueryErrorReason::TransactionFailed,
        })?;
    assert!(matches!(
        admin::delete_tenant(&connector, tenant).await,
        Err(FdbError::Tenant {
            reason: TenantErrorReason::RepairRequiresEmptyTenant { .. }
        })
    ));
    verify_ready(&connector, &[tenant]).await?;
    handle
        .transact_boxed(
            (),
            |transaction, _| {
                Box::pin(async move {
                    let key =
                        TenantDataKey::new(b"delete-integration/data").expect("fixed data key");
                    transaction.clear(key);
                    Ok::<(), foundationdb::FdbError>(())
                })
            },
            TenantTransactionPolicy::Write(WriteTxnPolicy::default()),
        )
        .await
        .map_err(|_| FdbError::Query {
            reason: reallyme_foundationdb_kit::FdbQueryErrorReason::TransactionFailed,
        })?;
    drop(handle);
    admin::delete_tenant(&connector, tenant).await?;
    assert!(!admin::tenant_exists(&connector, tenant).await?);
    Ok(())
}
