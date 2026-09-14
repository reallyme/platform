// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::Duration;

use reallyme_typesense_kit::{
    ConnectorBuildError, TypesenseConfig, TypesenseConfigError, TypesenseConnector,
    TypesenseEndpoint, TypesenseError,
};
use secrecy::SecretString;
use thiserror::Error;

const DEFAULT_INTEGRATION_ENDPOINT: &str = "http://127.0.0.1:8108";
const INTEGRATION_ENDPOINT_ENV: &str = "REALLYME_TYPESENSE_INTEGRATION_ENDPOINT";

#[derive(Debug, Error)]
enum IntegrationTestError {
    #[error("invalid integration test configuration")]
    Config(#[from] TypesenseConfigError),
    #[error("failed to build integration test connector")]
    Connector(#[from] ConnectorBuildError),
    #[error("typesense integration request failed")]
    Request(#[from] TypesenseError),
}

#[tokio::test]
#[ignore = "requires a reachable Typesense server"]
async fn connector_reports_live_typesense_ready() -> Result<(), IntegrationTestError> {
    let endpoint = match std::env::var(INTEGRATION_ENDPOINT_ENV) {
        Ok(value) => value,
        Err(_) => DEFAULT_INTEGRATION_ENDPOINT.to_owned(),
    };
    let config = TypesenseConfig::new(
        TypesenseEndpoint::parse(endpoint)?,
        // Typesense exposes `/health` without collection credentials. A non-empty
        // placeholder keeps connector construction identical to authenticated use.
        SecretString::new("health-check-placeholder".to_owned().into()),
        Duration::from_secs(2),
    )?
    .with_max_retries(0);
    let connector = TypesenseConnector::connect(config)?;

    let report = connector.health_report().await?;

    assert!(report.ready);
    Ok(())
}
