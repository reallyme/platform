// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typesense connector, request, schema, and document primitives.

mod client;
mod collection;
mod config;
mod connector;
mod document;
mod error;
mod field;
mod filter;
mod import;
mod pagination;
mod query;
mod response_body;
mod schema;
mod sort;

pub use client::{
    MultiSearchHit, MultiSearchRequest, MultiSearchRequestItem, MultiSearchResponse,
    MultiSearchResult, SearchRequest, SearchResultHit, SearchResults, TypesenseClient,
    TypesenseHealthReport,
};
pub use collection::{CollectionField, CollectionName, CollectionSchema};
pub use config::{
    TypesenseConfig, TypesenseConfigError, TypesenseConfigErrorReason, TypesenseEndpoint,
    TypesenseEndpointSelection,
};
pub use connector::{ConnectorBuildError, ConnectorBuildErrorReason, TypesenseConnector};
pub use document::{DocumentId, SearchDocument};
pub use error::{
    TypesenseError, TypesenseErrorCategory, TypesenseRequestReason, TypesenseResult,
    TypesenseTransportReason, TypesenseUpstreamReason,
};
pub use field::{SearchFieldName, SearchFields};
pub use filter::{FilterValue, SearchFilter, TextFilterValue};
pub use import::{
    ImportAction, ImportBatch, ImportFailureReason, ImportLine, ImportResult,
    parse_import_response_jsonl,
};
pub use pagination::{PageNumber, PageSize};
pub use query::SearchQuery;
pub use schema::FieldKind;
pub use sort::{SortBy, SortDirection, SortField, SortOrder};
