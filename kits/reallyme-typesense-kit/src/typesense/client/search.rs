// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::typesense::{PageNumber, PageSize, SearchFields, SearchFilter, SearchQuery, SortBy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;

/// Search request parameters sent to Typesense.
#[derive(Serialize)]
pub struct SearchRequest<'a> {
    q: &'a str,
    query_by: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter_by: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_by: Option<&'a str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    use_cache: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_ttl: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query_by_weights: Option<&'a str>,
    prefix: bool,
    page: u32,
    per_page: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_fields: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude_fields: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    highlight_fields: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_typos: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    drop_tokens_threshold: Option<f32>,
}

impl<'a> SearchRequest<'a> {
    /// Creates a search-as-you-type request.
    #[must_use]
    pub fn search_as_you_type(
        query: &'a SearchQuery,
        query_by: &'a SearchFields,
        page_size: PageSize,
        sort: Option<&'a SortBy>,
        filter: Option<&'a SearchFilter>,
    ) -> Self {
        Self {
            q: query.as_str(),
            query_by: query_by.to_query_by_parameter(),
            filter_by: filter.map(|value| Cow::Borrowed(value.to_filter_by_parameter())),
            sort_by: sort.map(SortBy::to_query_value),
            use_cache: true,
            cache_ttl: None,
            query_by_weights: None,
            prefix: true,
            page: PageNumber::FIRST.get(),
            per_page: page_size.get(),
            include_fields: None,
            exclude_fields: None,
            highlight_fields: None,
            num_typos: None,
            drop_tokens_threshold: None,
        }
    }

    /// Creates a search request using exact query matching.
    #[must_use]
    pub fn search_exact_match(
        query: &'a SearchQuery,
        query_by: &'a SearchFields,
        page_size: PageSize,
        sort: Option<&'a SortBy>,
        filter: Option<&'a SearchFilter>,
    ) -> Self {
        let mut request = Self::search_as_you_type(query, query_by, page_size, sort, filter);
        request.prefix = false;
        request
    }

    /// Enables or disables Typesense response cache usage.
    #[must_use]
    pub fn with_cache(mut self, use_cache: bool) -> Self {
        self.use_cache = use_cache;
        self
    }

    /// Sets the requested page.
    #[must_use]
    pub fn with_page(mut self, page: PageNumber) -> Self {
        self.page = page.get();
        self
    }

    /// Sets cache TTL in seconds.
    #[must_use]
    pub fn with_cache_ttl(mut self, cache_ttl: u32) -> Self {
        self.cache_ttl = Some(cache_ttl);
        self
    }

    /// Enables or disables prefix matching.
    #[must_use]
    pub fn with_prefix(mut self, prefix: bool) -> Self {
        self.prefix = prefix;
        self
    }

    /// Sets per-field query weighting.
    #[must_use]
    pub fn with_query_by_weights(mut self, query_by_weights: &'a str) -> Self {
        self.query_by_weights = Some(query_by_weights);
        self
    }

    /// Limits fields returned in the payload.
    #[must_use]
    pub fn with_include_fields(mut self, include_fields: &'a str) -> Self {
        self.include_fields = Some(include_fields);
        self
    }

    /// Excludes fields from the payload.
    #[must_use]
    pub fn with_exclude_fields(mut self, exclude_fields: &'a str) -> Self {
        self.exclude_fields = Some(exclude_fields);
        self
    }

    /// Explicitly sets highlighted fields.
    #[must_use]
    pub fn with_highlight_fields(mut self, highlight_fields: &'a str) -> Self {
        self.highlight_fields = Some(highlight_fields);
        self
    }

    /// Sets typos threshold.
    #[must_use]
    pub fn with_num_typos(mut self, num_typos: u8) -> Self {
        self.num_typos = Some(num_typos);
        self
    }

    /// Sets drop-tokens threshold.
    #[must_use]
    pub fn with_drop_tokens_threshold(mut self, drop_tokens_threshold: f32) -> Self {
        self.drop_tokens_threshold = Some(drop_tokens_threshold);
        self
    }
}

/// One search request item inside a merged multi-search payload.
#[derive(Serialize)]
pub struct MultiSearchRequestItem<'a> {
    collection: &'a str,
    q: &'a str,
    query_by: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter_by: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_by: Option<&'a str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    use_cache: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_ttl: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query_by_weights: Option<&'a str>,
    prefix: bool,
    page: u32,
    per_page: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_fields: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude_fields: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    highlight_fields: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_typos: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    drop_tokens_threshold: Option<f32>,
}

impl<'a> MultiSearchRequestItem<'a> {
    /// Creates one merged query.
    #[must_use]
    pub fn new(
        collection: &'a str,
        q: &'a str,
        query_by: &'a str,
        filter_by: Option<Cow<'a, str>>,
        sort_by: Option<&'a str>,
        page: u32,
        per_page: u8,
    ) -> Self {
        Self {
            collection,
            q,
            query_by,
            filter_by,
            sort_by,
            use_cache: true,
            cache_ttl: None,
            query_by_weights: None,
            prefix: true,
            page,
            per_page,
            include_fields: None,
            exclude_fields: None,
            highlight_fields: None,
            num_typos: None,
            drop_tokens_threshold: None,
        }
    }

    /// Converts a standard request into a merge-search request item.
    #[must_use]
    pub fn from_request(collection: &'a str, request: &'a SearchRequest<'a>) -> Self {
        Self {
            collection,
            q: request.q,
            query_by: request.query_by,
            filter_by: request.filter_by.clone(),
            sort_by: request.sort_by,
            use_cache: request.use_cache,
            cache_ttl: request.cache_ttl,
            query_by_weights: request.query_by_weights,
            prefix: request.prefix,
            page: request.page,
            per_page: request.per_page,
            include_fields: request.include_fields,
            exclude_fields: request.exclude_fields,
            highlight_fields: request.highlight_fields,
            num_typos: request.num_typos,
            drop_tokens_threshold: request.drop_tokens_threshold,
        }
    }
}

/// Typesense multi-search request payload.
#[derive(Serialize)]
pub struct MultiSearchRequest<'a> {
    searches: Vec<MultiSearchRequestItem<'a>>,
}

impl<'a> MultiSearchRequest<'a> {
    /// Creates a multi-search payload.
    #[must_use]
    pub fn new(searches: Vec<MultiSearchRequestItem<'a>>) -> Self {
        Self { searches }
    }

    /// Returns the list of queries.
    #[must_use]
    pub fn searches(&self) -> &[MultiSearchRequestItem<'a>] {
        &self.searches
    }
}

/// One multi-search result container.
#[derive(Deserialize)]
pub struct MultiSearchResponse {
    /// Search payloads in request order.
    #[serde(default)]
    pub results: Vec<MultiSearchResult>,
}

/// A request bucket returned from multi-search.
#[derive(Deserialize)]
pub struct MultiSearchResult {
    /// Matched hits for a sub-search request.
    #[serde(default)]
    pub hits: Vec<MultiSearchHit>,
    /// Number of matches found by Typesense before paging.
    #[serde(default)]
    pub found: u64,
    /// Current one-based result page.
    #[serde(default)]
    pub page: u32,
    /// Per-search HTTP-style failure code returned by Typesense.
    #[serde(default)]
    pub code: Option<u16>,
}

/// A search hit from multi-search.
#[derive(Deserialize)]
pub struct MultiSearchHit {
    /// Raw search document payload.
    pub document: Value,
    /// Opaque Typesense text-match score used only for relative ordering.
    #[serde(default)]
    pub text_match: Option<u64>,
}

/// Typesense search response.
#[derive(Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct SearchResults<T> {
    /// Matching hits.
    #[serde(default)]
    pub hits: Vec<SearchResultHit<T>>,
    /// Number of matches found by Typesense before paging.
    #[serde(default)]
    pub found: u64,
    /// Number of documents considered by the query engine.
    #[serde(default)]
    pub out_of: u64,
    /// Current page returned by Typesense.
    #[serde(default)]
    pub page: u32,
    /// Search latency in milliseconds as reported by Typesense.
    #[serde(default)]
    pub search_time_ms: u64,
    /// Optional facet counts returned by Typesense.
    #[serde(default)]
    pub facet_counts: Option<Vec<Value>>,
}

/// One Typesense search hit.
#[derive(Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct SearchResultHit<T> {
    /// Document payload.
    pub document: T,
}
