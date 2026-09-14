// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bulk import request and response helpers.

use serde::Deserialize;
use serde_json::Deserializer;

use crate::typesense::{
    DocumentId, SearchDocument, TypesenseError, TypesenseRequestReason, TypesenseTransportReason,
    error::TypesenseResult,
};

const MAX_DOCUMENTS_PER_BATCH: usize = 1_000;
const MAX_IMPORT_ERROR_MESSAGE_LENGTH: usize = 512;

/// Typesense import action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportAction {
    /// Create missing documents and update existing documents.
    Upsert,
    /// Create only missing documents.
    Create,
    /// Update only existing documents.
    Update,
    /// Create missing documents without altering existing fields omitted from the payload.
    Emplace,
}

impl ImportAction {
    /// Returns the Typesense wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Create => "create",
            Self::Update => "update",
            Self::Emplace => "emplace",
        }
    }
}

/// A batch of documents to import.
pub struct ImportBatch<T> {
    documents: Vec<T>,
}

impl<T> ImportBatch<T>
where
    T: SearchDocument,
{
    /// Creates a non-empty import batch.
    pub fn new(documents: Vec<T>) -> TypesenseResult<Self> {
        if documents.is_empty() {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::EmptyImportBatch,
            });
        }

        if documents.len() > MAX_DOCUMENTS_PER_BATCH {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::ImportBatchTooLarge,
            });
        }

        Ok(Self { documents })
    }

    /// Returns the documents in this batch.
    #[must_use]
    pub fn documents(&self) -> &[T] {
        &self.documents
    }
}

/// One line of the Typesense JSONL import response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportLine {
    /// Whether this line succeeded.
    pub success: bool,
    /// Document id returned by Typesense when available.
    pub id: Option<DocumentId>,
    /// Stable failure classification when Typesense rejected this document.
    pub failure_reason: Option<ImportFailureReason>,
    /// Bounded length of the upstream failure message when present.
    pub failure_message_length: Option<u16>,
    /// Whether Typesense included a document payload in the failure response.
    pub had_document_payload: bool,
}

/// Stable classification of per-document import failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFailureReason {
    /// Typesense reported a duplicate or already-existing document.
    DocumentAlreadyExists,
    /// Typesense reported a missing document for update-only operations.
    DocumentNotFound,
    /// Typesense rejected one or more fields in the document.
    InvalidField,
    /// Typesense rejected the request due to collection schema constraints.
    SchemaViolation,
    /// Typesense rejected the request due to authorization.
    AuthorizationFailed,
    /// Typesense reported rate limiting.
    RateLimited,
    /// Typesense rejected the request because the collection was unavailable.
    CollectionUnavailable,
    /// Typesense rejected the request because the payload was too large.
    PayloadTooLarge,
    /// Typesense rejected the request as invalid without a more specific category.
    InvalidRequest,
    /// Typesense returned an unclassified failure message.
    Unknown,
}

/// Parsed import result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportResult {
    /// Per-document import outcomes.
    pub lines: Vec<ImportLine>,
}

/// Parses the newline-delimited Typesense import response body.
pub fn parse_import_response_jsonl(body: &str) -> TypesenseResult<ImportResult> {
    let mut lines = Vec::new();

    for raw_line in body.lines() {
        if raw_line.trim().is_empty() {
            continue;
        }

        let mut deserializer = Deserializer::from_str(raw_line);
        let line = RawImportLine::deserialize(&mut deserializer).map_err(|_| {
            TypesenseError::Transport {
                reason: TypesenseTransportReason::InvalidResponseBody,
            }
        })?;

        if deserializer.end().is_err() {
            return Err(TypesenseError::Transport {
                reason: TypesenseTransportReason::InvalidResponseBody,
            });
        }

        lines.push(ImportLine::try_from(line)?);
    }

    Ok(ImportResult { lines })
}

#[derive(Deserialize)]
struct RawImportLine {
    success: bool,
    id: Option<String>,
    error: Option<String>,
    document: Option<serde_json::Value>,
}

impl TryFrom<RawImportLine> for ImportLine {
    type Error = TypesenseError;

    fn try_from(value: RawImportLine) -> Result<Self, Self::Error> {
        let id =
            value
                .id
                .map(DocumentId::parse)
                .transpose()
                .map_err(|_| TypesenseError::Transport {
                    reason: TypesenseTransportReason::InvalidResponseBody,
                })?;

        Ok(Self {
            success: value.success,
            id,
            failure_reason: value.error.as_deref().map(classify_import_failure_reason),
            failure_message_length: value.error.as_deref().map(bounded_message_length),
            had_document_payload: value.document.is_some(),
        })
    }
}

fn bounded_message_length(message: &str) -> u16 {
    let bounded = message.len().min(MAX_IMPORT_ERROR_MESSAGE_LENGTH);
    u16::try_from(bounded).unwrap_or(u16::MAX)
}

fn classify_import_failure_reason(message: &str) -> ImportFailureReason {
    let normalized = message.to_ascii_lowercase();

    if normalized.contains("already exists") || normalized.contains("duplicate") {
        ImportFailureReason::DocumentAlreadyExists
    } else if normalized.contains("not found") || normalized.contains("could not find a document") {
        ImportFailureReason::DocumentNotFound
    } else if normalized.contains("schema")
        || normalized.contains("facet")
        || normalized.contains("sort field")
    {
        ImportFailureReason::SchemaViolation
    } else if normalized.contains("field") {
        ImportFailureReason::InvalidField
    } else if normalized.contains("unauthorized")
        || normalized.contains("forbidden")
        || normalized.contains("api key")
    {
        ImportFailureReason::AuthorizationFailed
    } else if normalized.contains("rate limit") || normalized.contains("too many requests") {
        ImportFailureReason::RateLimited
    } else if normalized.contains("collection") {
        ImportFailureReason::CollectionUnavailable
    } else if normalized.contains("payload too large") || normalized.contains("request too large") {
        ImportFailureReason::PayloadTooLarge
    } else if normalized.contains("invalid") || normalized.contains("malformed") {
        ImportFailureReason::InvalidRequest
    } else {
        ImportFailureReason::Unknown
    }
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
