// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::Duration;

use reallyme_typesense_kit::{
    CollectionField, CollectionName, CollectionSchema, ConnectorBuildError, DocumentId, FieldKind,
    ImportAction, ImportBatch, PageSize, SearchDocument, SearchFieldName, SearchFields,
    SearchQuery, SearchRequest, TypesenseClient, TypesenseConfig, TypesenseConfigError,
    TypesenseConnector, TypesenseEndpoint, TypesenseError,
};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_INTEGRATION_ENDPOINT: &str = "http://127.0.0.1:8108";
const INTEGRATION_ENDPOINT_ENV: &str = "REALLYME_TYPESENSE_INTEGRATION_ENDPOINT";
const INTEGRATION_API_KEY_ENV: &str = "REALLYME_TYPESENSE_INTEGRATION_API_KEY";
const INTEGRATION_COLLECTION: &str = "platform_typesense_30_integration";

#[derive(Debug, Error)]
enum IntegrationTestError {
    #[error("invalid integration test configuration")]
    Config(#[from] TypesenseConfigError),
    #[error("failed to build integration test connector")]
    Connector(#[from] ConnectorBuildError),
    #[error("typesense integration request failed")]
    Request(#[from] TypesenseError),
    #[error("typesense collection response violated the integration contract")]
    CollectionContract,
    #[error("typesense import response violated the integration contract")]
    ImportContract,
    #[error("typesense search response violated the integration contract")]
    SearchContract,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct IntegrationDocument {
    id: DocumentId,
    title: String,
}

impl SearchDocument for IntegrationDocument {
    fn document_id(&self) -> &DocumentId {
        &self.id
    }
}

#[tokio::test]
#[ignore = "requires a reachable Typesense server"]
async fn connector_exercises_typesense_30_collection_import_and_search()
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
    let client = connector.client();
    let collection = CollectionName::parse(INTEGRATION_COLLECTION)?;

    let report = connector.health_report().await?;
    if !report.ready {
        return Err(IntegrationTestError::CollectionContract);
    }

    remove_stale_collection(client, &collection).await?;
    let exercise_result = exercise_collection_round_trip(client, &collection).await;
    let cleanup_result = client.delete_collection(&collection).await;

    match (exercise_result, cleanup_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(IntegrationTestError::Request(error)),
        (Ok(()), Ok(())) => Ok(()),
    }
}

async fn remove_stale_collection(
    client: &TypesenseClient,
    collection: &CollectionName,
) -> Result<(), IntegrationTestError> {
    match client.delete_collection(collection).await {
        Ok(()) => Ok(()),
        Err(TypesenseError::Upstream { status, .. }) if status == 404 => Ok(()),
        Err(error) => Err(IntegrationTestError::Request(error)),
    }
}

async fn exercise_collection_round_trip(
    client: &TypesenseClient,
    collection: &CollectionName,
) -> Result<(), IntegrationTestError> {
    let title_field = SearchFieldName::parse("title")?;
    let schema = CollectionSchema::new(
        collection.clone(),
        vec![CollectionField::new(title_field.clone(), FieldKind::String)],
        None,
    )?;
    let created = client.create_collection(&schema).await?;
    if created.name != *collection {
        return Err(IntegrationTestError::CollectionContract);
    }

    let document = IntegrationDocument {
        id: DocumentId::parse("typesense-30-document")?,
        title: "Platform compatibility probe".to_owned(),
    };
    let batch = ImportBatch::new(vec![document.clone()])?;
    let imported = client
        .import_documents(collection, ImportAction::Create, &batch)
        .await?;
    if imported.lines.len() != 1 || !imported.lines[0].success {
        return Err(IntegrationTestError::ImportContract);
    }

    let query = SearchQuery::parse("compatibility")?;
    let fields = SearchFields::new(vec![title_field])?;
    let page_size = PageSize::parse(1, PageSize::MAX_SEARCH_AS_YOU_TYPE)?;
    let request =
        SearchRequest::search_exact_match(&query, &fields, page_size, None, None).with_cache(false);
    let results = client
        .search::<IntegrationDocument>(collection, &request)
        .await?;
    if results.found != 1 || results.hits.len() != 1 || results.hits[0].document != document {
        return Err(IntegrationTestError::SearchContract);
    }

    Ok(())
}
