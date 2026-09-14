// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed error model for Typesense operations.

use reqwest::StatusCode;
use thiserror::Error;

/// Result alias for Typesense operations.
pub type TypesenseResult<T> = Result<T, TypesenseError>;

/// Main Typesense error type.
#[derive(Debug, Error)]
pub enum TypesenseError {
    /// The caller supplied invalid request parameters.
    #[error("invalid typesense request")]
    InvalidRequest {
        /// Stable machine-readable reason.
        reason: TypesenseRequestReason,
    },
    /// Typesense returned an unsuccessful HTTP status.
    #[error("typesense upstream returned an error")]
    Upstream {
        /// Stable machine-readable reason.
        reason: TypesenseUpstreamReason,
        /// HTTP status returned by Typesense.
        status: StatusCode,
    },
    /// The request could not be completed or decoded.
    #[error("typesense transport failure")]
    Transport {
        /// Stable machine-readable reason.
        reason: TypesenseTransportReason,
    },
}

impl TypesenseError {
    /// Returns the broad failure category for logging and metrics.
    #[must_use]
    pub const fn category(&self) -> TypesenseErrorCategory {
        match self {
            Self::InvalidRequest { .. } => TypesenseErrorCategory::InvalidRequest,
            Self::Upstream { .. } => TypesenseErrorCategory::Upstream,
            Self::Transport { .. } => TypesenseErrorCategory::Transport,
        }
    }
}

/// Broad failure category for a Typesense operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypesenseErrorCategory {
    /// Caller-controlled request validation failure.
    InvalidRequest,
    /// Typesense responded but rejected or failed the request.
    Upstream,
    /// Network, timeout, or response decoding failure.
    Transport,
}

/// Stable reason for invalid request failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypesenseRequestReason {
    /// A search query was empty or became empty after normalization.
    EmptyQuery,
    /// A search query was shorter than the accepted minimum.
    QueryTooShort,
    /// A search query exceeded the accepted maximum.
    QueryTooLong,
    /// A search query contained unsupported characters.
    QueryContainsInvalidCharacters,
    /// A search request did not name any query fields.
    EmptyQueryFields,
    /// A search request named too many query fields.
    TooManyQueryFields,
    /// A search, sort, or filter field name was invalid.
    InvalidSearchFieldName,
    /// A page size of zero was requested.
    ZeroPageSize,
    /// A page size exceeded the configured maximum.
    PageSizeTooLarge,
    /// A collection schema did not contain any fields.
    EmptyCollectionFields,
    /// An existing collection field cannot satisfy the requested schema.
    IncompatibleCollectionSchema,
    /// A vector field used an unsupported kind or dimension count.
    InvalidVectorDimensions,
    /// A sort clause did not contain any sort orders.
    EmptySortOrders,
    /// A sort clause exceeded the supported number of sort orders.
    TooManySortOrders,
    /// A page number of zero was requested.
    ZeroPageNumber,
    /// A collection name was empty or syntactically invalid.
    InvalidCollectionName,
    /// A document id was empty or syntactically invalid.
    InvalidDocumentId,
    /// A filter value could not be represented safely.
    InvalidFilterValue,
    /// An import batch did not contain any documents.
    EmptyImportBatch,
    /// An import batch exceeded the supported document count.
    ImportBatchTooLarge,
    /// A backfill batch size of zero was requested.
    EmptyBackfillBatchSize,
    /// A backfill batch size exceeded the supported maximum.
    BackfillBatchTooLarge,
}

/// Stable reason for upstream HTTP status failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypesenseUpstreamReason {
    /// The configured API key is missing required authorization.
    AuthorizationFailed,
    /// The requested collection was not available.
    CollectionUnavailable,
    /// A requested document was not found in the collection.
    DocumentNotFound,
    /// Typesense returned a conflict for the requested mutation.
    Conflict,
    /// Typesense rate limited the caller.
    RateLimited,
    /// Typesense returned a transient server status.
    ServiceUnavailable,
    /// Typesense returned an unmapped unsuccessful status.
    UnexpectedStatus,
}

/// Stable reason for network and decoding failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypesenseTransportReason {
    /// The HTTP request could not be sent or completed.
    RequestFailed,
    /// The response body was not valid for the expected shape.
    InvalidResponseBody,
    /// A request document could not be serialized safely.
    RequestSerializationFailed,
}

/// Maps a non-success Typesense status into a typed error.
#[must_use]
pub fn map_status(status: StatusCode) -> TypesenseError {
    let reason = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            TypesenseUpstreamReason::AuthorizationFailed
        }
        StatusCode::NOT_FOUND => TypesenseUpstreamReason::CollectionUnavailable,
        StatusCode::CONFLICT => TypesenseUpstreamReason::Conflict,
        StatusCode::TOO_MANY_REQUESTS => TypesenseUpstreamReason::RateLimited,
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT => {
            TypesenseUpstreamReason::ServiceUnavailable
        }
        _ => TypesenseUpstreamReason::UnexpectedStatus,
    };

    TypesenseError::Upstream { reason, status }
}
