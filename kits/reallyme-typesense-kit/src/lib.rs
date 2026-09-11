// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! First-class Typesense infrastructure primitives for ReallyMe services.
//!
//! This kit owns reusable endpoint and credential configuration, HTTP
//! connection pooling, retry and failover policy, typed search requests,
//! collection/schema management, document imports, filters, sorting, and
//! pagination. Search products keep their document schemas, ranking policy,
//! and response models in app-owned adapters.

pub mod typesense;

pub use typesense::{
    CollectionField, CollectionName, CollectionSchema, ConnectorBuildError,
    ConnectorBuildErrorReason, DocumentId, FieldKind, FilterValue, ImportAction, ImportBatch,
    ImportFailureReason, ImportLine, ImportResult, MultiSearchHit, MultiSearchRequest,
    MultiSearchRequestItem, MultiSearchResponse, MultiSearchResult, PageNumber, PageSize,
    SearchDocument, SearchFieldName, SearchFields, SearchFilter, SearchQuery, SearchRequest,
    SearchResultHit, SearchResults, SortBy, SortDirection, SortField, SortOrder, TextFilterValue,
    TypesenseClient, TypesenseConfig, TypesenseConfigError, TypesenseConfigErrorReason,
    TypesenseConnector, TypesenseEndpoint, TypesenseEndpointSelection, TypesenseError,
    TypesenseErrorCategory, TypesenseHealthReport, TypesenseRequestReason, TypesenseResult,
    TypesenseTransportReason, TypesenseUpstreamReason, parse_import_response_jsonl,
};
