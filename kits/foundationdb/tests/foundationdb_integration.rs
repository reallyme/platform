// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![deny(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use reallyme_foundationdb_kit::fdb::transaction::{TenantTransactionPolicy, WriteTxnPolicy};
use reallyme_foundationdb_kit::{
    FdbConfig, FdbResult, FoundationDbConnector, FoundationDbTenantName, TenantDataKey,
    verify_ready,
};

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
                    let key = TenantDataKey::new(b"kit-integration/data")
                        .expect("integration data key must remain valid");
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
