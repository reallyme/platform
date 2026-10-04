// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::Duration;

use reallyme_typesense_kit::{
    CollectionName, ConnectorBuildError, TypesenseConfig, TypesenseConfigError, TypesenseConnector,
    TypesenseEndpoint, TypesenseError,
};
use secrecy::SecretString;
use thiserror::Error;

const DEFAULT_INTEGRATION_ENDPOINT: &str = "http://127.0.0.1:8108";
const INTEGRATION_ENDPOINT_ENV: &str = "REALLYME_TYPESENSE_INTEGRATION_ENDPOINT";
const INTEGRATION_API_KEY_ENV: &str = "REALLYME_TYPESENSE_INTEGRATION_API_KEY";

#[derive(Debug, Error)]
enum IntegrationTestError {
    #[error("invalid integration test configuration")]
    Config(#[from] TypesenseConfigError),
    #[error("failed to build integration test connector")]
    Connector(#[from] ConnectorBuildError),
    #[error("typesense integration request failed")]
    Request(#[from] TypesenseError),
    #[error("invalid typesense integration collection name")]
    Collection,
    #[error("authenticated typesense request returned an unexpected status")]
    AuthenticatedRequest,
}

#[tokio::test]
#[ignore = "requires a reachable Typesense server"]
async fn connector_checks_live_health_and_authenticated_collection_read()
-> Result<(), IntegrationTestError> {
    let endpoint = match std::env::var(INTEGRATION_ENDPOINT_ENV) {
        Ok(value) => value,
        Err(_) => DEFAULT_INTEGRATION_ENDPOINT.to_owned(),
    };
    let config = TypesenseConfig::new(
        TypesenseEndpoint::parse(endpoint)?,
        SecretString::new(
            std::env::var(INTEGRATION_API_KEY_ENV)
                .unwrap_or_else(|_| "xyz".to_owned())
                .into(),
        ),
        Duration::from_secs(2),
    )?
    .with_max_retries(0);
    let connector = TypesenseConnector::connect(config)?;

    let report = connector.health_report().await?;

    assert!(report.ready);
    // A missing collection is a safe authenticated read: 404 proves the API
    // key was accepted, while 401/403 would expose credential wiring errors.
    let missing = CollectionName::parse("platform-integration-missing")
        .map_err(|_| IntegrationTestError::Collection)?;
    match connector.client().get_collection(&missing).await {
        Err(TypesenseError::Upstream { status, .. }) if status == 404 => {}
        _ => return Err(IntegrationTestError::AuthenticatedRequest),
    }
    Ok(())
}
