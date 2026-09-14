// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use reallyme_foundationdb_kit::{
    FdbConfig, FdbResult, FoundationDbConnector, FoundationDbTenantName, verify_ready,
};

const INTEGRATION_ENABLED_ENV: &str = "REALLYME_RUN_FOUNDATIONDB_INTEGRATION";
const INTEGRATION_TENANT: &str = "foundationdb-kit-integration";

#[tokio::test]
async fn live_connector_proves_cluster_and_tenant_readiness() -> FdbResult<()> {
    if std::env::var(INTEGRATION_ENABLED_ENV).as_deref() != Ok("1") {
        eprintln!("SKIP: set REALLYME_RUN_FOUNDATIONDB_INTEGRATION=1 for live FoundationDB tests");
        return Ok(());
    }

    let config = FdbConfig::from_env()?;
    let connector = FoundationDbConnector::connect(&config)?;
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
    Ok(())
}
