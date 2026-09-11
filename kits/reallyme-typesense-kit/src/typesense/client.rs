// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! HTTP client wrapper for Typesense operations.
//!
//! `TypesenseClient` is cheap to clone because it wraps a shared
//! `reqwest::Client` with connection pooling. Keep this handle in an `Arc`
//! only when shared mutable state is needed; clones are the intended sharing
//! pattern for normal request fan-out.

mod retry;
mod search;
use super::response_body::{decode_json, read_body};
use bytes::Bytes;
use reqwest::{Client, StatusCode};
pub use search::{
    MultiSearchHit, MultiSearchRequest, MultiSearchRequestItem, MultiSearchResponse,
    MultiSearchResult, SearchRequest, SearchResultHit, SearchResults,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::convert::TryFrom;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::typesense::{
    CollectionName, DocumentId, ImportAction, ImportBatch, ImportResult, SearchDocument, SortField,
    TypesenseConfig, TypesenseEndpoint, TypesenseEndpointSelection, TypesenseError,
    TypesenseTransportReason, TypesenseUpstreamReason, error::TypesenseResult,
    parse_import_response_jsonl,
};

const METRIC_REQUESTS_TOTAL: &str = "reallyme_typesense_kit_requests_total";
const METRIC_REQUEST_DURATION_SECONDS: &str = "reallyme_typesense_kit_request_duration_seconds";
const METRIC_REQUEST_ERRORS_TOTAL: &str = "reallyme_typesense_kit_request_errors_total";
const METRIC_REQUEST_RETRIES_TOTAL: &str = "reallyme_typesense_kit_request_retries_total";
const METRIC_REQUESTS_IN_FLIGHT: &str = "reallyme_typesense_kit_requests_in_flight";
const LABEL_OPERATION: &str = "operation";
const LABEL_OUTCOME: &str = "outcome";
const DEFAULT_RETRY_ENTROPY_SEED: u64 = 0x9E37_79B9_7F4A_7C15;
static NEXT_RETRY_ENTROPY_SEED: AtomicU64 = AtomicU64::new(DEFAULT_RETRY_ENTROPY_SEED);

#[derive(Serialize)]
struct CollectionSchemaRequest<'a> {
    name: &'a CollectionName,
    fields: &'a [crate::typesense::CollectionField],
    default_sorting_field: Option<&'a SortField>,
}

#[derive(Serialize)]
struct CollectionSchemaUpdateRequest<'a> {
    fields: &'a [crate::typesense::CollectionField],
}

impl<'a> From<&'a crate::typesense::CollectionSchema> for CollectionSchemaRequest<'a> {
    fn from(schema: &'a crate::typesense::CollectionSchema) -> Self {
        Self {
            name: &schema.name,
            fields: &schema.fields,
            default_sorting_field: schema.default_sorting_field.as_ref(),
        }
    }
}

/// Low-cardinality readiness snapshot returned by the Typesense connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypesenseHealthReport {
    /// Whether the selected Typesense node reported itself ready.
    pub ready: bool,
}

/// Reusable Typesense client.
pub struct TypesenseClient {
    http_client: Client,
    endpoints: Vec<TypesenseEndpoint>,
    endpoint_selection: TypesenseEndpointSelection,
    endpoint_cursor: AtomicUsize,
    search_request_timeout: Duration,
    import_request_timeout: Duration,
    max_retries: u8,
    retry_initial_delay: Duration,
    retry_max_delay: Duration,
    retry_jitter_percent: u8,
    retry_entropy: AtomicU64,
}

impl Clone for TypesenseClient {
    fn clone(&self) -> Self {
        Self {
            http_client: self.http_client.clone(),
            endpoints: self.endpoints.clone(),
            endpoint_selection: self.endpoint_selection,
            endpoint_cursor: AtomicUsize::new(self.endpoint_cursor.load(Ordering::Relaxed)),
            search_request_timeout: self.search_request_timeout,
            import_request_timeout: self.import_request_timeout,
            max_retries: self.max_retries,
            retry_initial_delay: self.retry_initial_delay,
            retry_max_delay: self.retry_max_delay,
            retry_jitter_percent: self.retry_jitter_percent,
            retry_entropy: AtomicU64::new(self.retry_entropy.load(Ordering::Relaxed)),
        }
    }
}

impl TypesenseClient {
    /// Creates a client from a configured HTTP client and endpoint configuration.
    #[must_use]
    pub fn new(http_client: Client, config: TypesenseConfig) -> Self {
        let request_timeout = config.request_timeout();
        let import_request_timeout = config
            .import_request_timeout()
            .unwrap_or_else(|| request_timeout.checked_mul(3).unwrap_or(request_timeout));

        Self {
            http_client,
            endpoints: config.endpoints().to_vec(),
            endpoint_selection: config.endpoint_selection(),
            endpoint_cursor: AtomicUsize::new(0),
            search_request_timeout: request_timeout,
            import_request_timeout,
            max_retries: config.max_retries(),
            retry_initial_delay: config.retry_initial_delay(),
            retry_max_delay: config.retry_max_delay(),
            retry_jitter_percent: config.retry_jitter_percent(),
            retry_entropy: AtomicU64::new(initial_retry_entropy_seed()),
        }
    }

    /// Checks whether a Typesense node is ready to accept requests.
    pub async fn health_report(&self) -> TypesenseResult<TypesenseHealthReport> {
        #[derive(Deserialize)]
        struct HealthResponse {
            ok: bool,
        }

        let response = self
            .execute_with_retries(
                self.search_request_timeout,
                |endpoint| self.http_client.get(self.health_url(endpoint)),
                RequestKind::Health,
            )
            .await?;
        let health = decode_json::<HealthResponse>(response).await?;
        if !health.ok {
            return Err(TypesenseError::Upstream {
                reason: TypesenseUpstreamReason::ServiceUnavailable,
                status: StatusCode::SERVICE_UNAVAILABLE,
            });
        }

        Ok(TypesenseHealthReport { ready: true })
    }

    /// Checks readiness without returning the health snapshot.
    pub async fn health_check(&self) -> TypesenseResult<()> {
        self.health_report().await.map(|_report| ())
    }

    /// Performs a typed search against a collection.
    pub async fn search<T>(
        &self,
        collection: &CollectionName,
        request: &SearchRequest<'_>,
    ) -> TypesenseResult<SearchResults<T>>
    where
        T: DeserializeOwned,
    {
        let response = self
            .execute_with_retries(
                self.search_request_timeout,
                |endpoint| {
                    self.http_client
                        .get(self.collection_documents_search_url(endpoint, collection))
                        .query(request)
                },
                RequestKind::Search,
            )
            .await?;

        decode_json::<SearchResults<T>>(response).await
    }

    /// Imports one batch of documents using Typesense JSONL bulk import.
    pub async fn import_documents<T>(
        &self,
        collection: &CollectionName,
        action: ImportAction,
        batch: &ImportBatch<T>,
    ) -> TypesenseResult<ImportResult>
    where
        T: serde::Serialize + SearchDocument,
    {
        let mut payload = String::new();
        for document in batch.documents() {
            let encoded =
                serde_json::to_string(document).map_err(|_| TypesenseError::Transport {
                    reason: TypesenseTransportReason::RequestSerializationFailed,
                })?;

            payload.push_str(&encoded);
            payload.push('\n');
        }
        let payload = Bytes::from(payload);

        let response = self
            .execute_with_retries(
                self.import_request_timeout,
                |endpoint| {
                    self.http_client
                        .post(self.collection_documents_import_url(endpoint, collection))
                        .query(&[("action", action.as_str())])
                        .header("Content-Type", "application/x-ndjson")
                        .body(payload.clone())
                },
                RequestKind::Collection,
            )
            .await?;

        let body = read_body(response).await?;
        let text = std::str::from_utf8(&body).map_err(|_| TypesenseError::Transport {
            reason: TypesenseTransportReason::InvalidResponseBody,
        })?;
        parse_import_response_jsonl(text)
    }

    /// Executes a multi-search request.
    pub async fn multi_search(
        &self,
        request: &MultiSearchRequest<'_>,
    ) -> TypesenseResult<MultiSearchResponse> {
        let response = self
            .execute_with_retries(
                self.search_request_timeout,
                |endpoint| {
                    self.http_client
                        .post(self.multi_search_url(endpoint))
                        .json(request)
                },
                RequestKind::Search,
            )
            .await?;

        decode_json::<MultiSearchResponse>(response).await
    }

    /// Creates one collection from a typed schema.
    pub async fn create_collection(
        &self,
        schema: &crate::typesense::CollectionSchema,
    ) -> TypesenseResult<crate::typesense::CollectionSchema> {
        let request = CollectionSchemaRequest::from(schema);
        let response = self
            .execute_with_retries(
                self.search_request_timeout,
                |endpoint| {
                    self.http_client
                        .post(self.collections_url(endpoint))
                        .json(&request)
                },
                RequestKind::CreateCollection,
            )
            .await?;

        let created = decode_json::<crate::typesense::CollectionSchema>(response).await?;

        if let Some(alias) = schema.alias.as_ref() {
            self.upsert_collection_alias(alias, &created.name).await?;
        }

        Ok(created)
    }

    /// Fetches one collection schema by name.
    pub async fn get_collection(
        &self,
        collection: &CollectionName,
    ) -> TypesenseResult<crate::typesense::CollectionSchema> {
        let response = self
            .execute_with_retries(
                self.search_request_timeout,
                |endpoint| {
                    self.http_client
                        .get(self.collection_url(endpoint, collection))
                },
                RequestKind::GetCollection,
            )
            .await?;

        decode_json::<crate::typesense::CollectionSchema>(response).await
    }

    /// Updates one existing collection schema.
    pub async fn update_collection(
        &self,
        collection: &CollectionName,
        schema: &crate::typesense::CollectionSchema,
    ) -> TypesenseResult<crate::typesense::CollectionSchema> {
        let request = CollectionSchemaUpdateRequest {
            fields: &schema.fields,
        };
        self.execute_with_retries(
            self.search_request_timeout,
            |endpoint| {
                self.http_client
                    .patch(self.collection_url(endpoint, collection))
                    .json(&request)
            },
            RequestKind::UpdateCollection,
        )
        .await?;

        if let Some(alias) = schema.alias.as_ref() {
            self.upsert_collection_alias(alias, collection).await?;
        }

        // Typesense returns only the changed field list for schema patches,
        // unlike collection creation. Fetching the complete schema keeps this
        // method's return contract stable and verifies the mutation.
        self.get_collection(collection).await
    }

    /// Deletes one collection.
    pub async fn delete_collection(&self, collection: &CollectionName) -> TypesenseResult<()> {
        self.execute_with_retries(
            self.search_request_timeout,
            |endpoint| {
                self.http_client
                    .delete(self.collection_url(endpoint, collection))
            },
            RequestKind::DeleteCollection,
        )
        .await?;

        Ok(())
    }

    /// Creates or updates one collection alias to point to a collection.
    pub async fn upsert_collection_alias(
        &self,
        alias: &CollectionName,
        collection: &CollectionName,
    ) -> TypesenseResult<()> {
        #[derive(Serialize)]
        struct AliasRequest<'a> {
            collection_name: &'a str,
        }

        self.execute_with_retries(
            self.search_request_timeout,
            |endpoint| {
                self.http_client
                    .put(self.collection_alias_url(endpoint, alias))
                    .json(&AliasRequest {
                        collection_name: collection.as_str(),
                    })
            },
            RequestKind::UpsertAlias,
        )
        .await?;

        Ok(())
    }

    /// Deletes one collection alias.
    pub async fn delete_collection_alias(&self, alias: &CollectionName) -> TypesenseResult<()> {
        self.execute_with_retries(
            self.search_request_timeout,
            |endpoint| {
                self.http_client
                    .delete(self.collection_alias_url(endpoint, alias))
            },
            RequestKind::DeleteAlias,
        )
        .await?;

        Ok(())
    }

    /// Upserts one document into a collection.
    ///
    /// The document body must contain an `id` field that Typesense uses for identity.
    pub async fn upsert_document<T>(
        &self,
        collection: &CollectionName,
        document: &T,
    ) -> TypesenseResult<()>
    where
        T: Serialize + ?Sized,
    {
        self.execute_with_retries(
            self.search_request_timeout,
            |endpoint| {
                self.http_client
                    .post(self.collection_documents_url(endpoint, collection))
                    .query(&[("action", "upsert")])
                    .json(document)
            },
            RequestKind::UpsertDocument,
        )
        .await?;

        Ok(())
    }

    /// Fetches one document by its validated identifier.
    pub async fn get_document<T>(
        &self,
        collection: &CollectionName,
        document_id: &DocumentId,
    ) -> TypesenseResult<T>
    where
        T: DeserializeOwned,
    {
        let response = self
            .execute_with_retries(
                self.search_request_timeout,
                |endpoint| {
                    self.http_client.get(self.collection_document_url(
                        endpoint,
                        collection,
                        document_id,
                    ))
                },
                RequestKind::GetDocument,
            )
            .await?;

        decode_json::<T>(response).await
    }

    /// Deletes one document from a collection.
    pub async fn delete_document(
        &self,
        collection: &CollectionName,
        document_id: &DocumentId,
    ) -> TypesenseResult<()> {
        self.execute_with_retries(
            self.search_request_timeout,
            |endpoint| {
                self.http_client.delete(self.collection_document_url(
                    endpoint,
                    collection,
                    document_id,
                ))
            },
            RequestKind::DeleteDocument,
        )
        .await?;

        Ok(())
    }

    fn collection_documents_search_url(
        &self,
        endpoint: &TypesenseEndpoint,
        collection: &CollectionName,
    ) -> String {
        format!(
            "{}/collections/{}/documents/search",
            endpoint.as_str(),
            collection.as_str()
        )
    }

    fn health_url(&self, endpoint: &TypesenseEndpoint) -> String {
        format!("{}/health", endpoint.as_str())
    }

    fn collection_documents_url(
        &self,
        endpoint: &TypesenseEndpoint,
        collection: &CollectionName,
    ) -> String {
        format!(
            "{}/collections/{}/documents",
            endpoint.as_str(),
            collection.as_str()
        )
    }

    fn collection_document_url(
        &self,
        endpoint: &TypesenseEndpoint,
        collection: &CollectionName,
        document_id: &DocumentId,
    ) -> String {
        format!(
            "{}/collections/{}/documents/{}",
            endpoint.as_str(),
            collection.as_str(),
            document_id.as_str()
        )
    }

    fn collection_documents_import_url(
        &self,
        endpoint: &TypesenseEndpoint,
        collection: &CollectionName,
    ) -> String {
        format!(
            "{}/collections/{}/documents/import",
            endpoint.as_str(),
            collection.as_str()
        )
    }

    fn multi_search_url(&self, endpoint: &TypesenseEndpoint) -> String {
        format!("{}/multi_search", endpoint.as_str())
    }

    fn collections_url(&self, endpoint: &TypesenseEndpoint) -> String {
        format!("{}/collections", endpoint.as_str())
    }

    fn collection_url(&self, endpoint: &TypesenseEndpoint, collection: &CollectionName) -> String {
        format!("{}/collections/{}", endpoint.as_str(), collection.as_str())
    }

    fn collection_alias_url(&self, endpoint: &TypesenseEndpoint, alias: &CollectionName) -> String {
        format!("{}/aliases/{}", endpoint.as_str(), alias.as_str())
    }
}

fn initial_retry_entropy_seed() -> u64 {
    let counter_seed = NEXT_RETRY_ENTROPY_SEED.fetch_add(0xA076_1D64_78BD_642F, Ordering::Relaxed);
    let time_seed = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let nanos = duration.as_nanos();
            let lower = nanos & u128::from(u64::MAX);
            u64::try_from(lower).unwrap_or(DEFAULT_RETRY_ENTROPY_SEED)
        }
        Err(_) => DEFAULT_RETRY_ENTROPY_SEED,
    };
    let mixed = advance_retry_entropy(counter_seed ^ time_seed.rotate_left(17));
    if mixed == 0 {
        DEFAULT_RETRY_ENTROPY_SEED
    } else {
        mixed
    }
}

fn advance_retry_entropy(mut value: u64) -> u64 {
    if value == 0 {
        value = DEFAULT_RETRY_ENTROPY_SEED;
    }
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    if value == 0 {
        DEFAULT_RETRY_ENTROPY_SEED
    } else {
        value
    }
}

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    Health,
    Collection,
    Search,
    CreateCollection,
    GetCollection,
    UpdateCollection,
    DeleteCollection,
    UpsertAlias,
    DeleteAlias,
    UpsertDocument,
    GetDocument,
    DeleteDocument,
}

impl RequestKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Collection => "collection",
            Self::Search => "search",
            Self::CreateCollection => "create_collection",
            Self::GetCollection => "get_collection",
            Self::UpdateCollection => "update_collection",
            Self::DeleteCollection => "delete_collection",
            Self::UpsertAlias => "upsert_alias",
            Self::DeleteAlias => "delete_alias",
            Self::UpsertDocument => "upsert_document",
            Self::GetDocument => "get_document",
            Self::DeleteDocument => "delete_document",
        }
    }
}
