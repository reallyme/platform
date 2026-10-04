// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::typesense::error::TypesenseResult;
use crate::typesense::{
    CollectionName, PageNumber, PageSize, SearchFields, SearchFilter, SearchQuery,
    SearchQueryWeights, SortBy, TypesenseError, TypesenseRequestReason,
};
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
    pub fn with_query_by_weights(mut self, query_by_weights: &'a SearchQueryWeights) -> Self {
        self.query_by_weights = Some(query_by_weights.as_str());
        self
    }

    /// Limits fields returned in the payload.
    #[must_use]
    pub fn with_include_fields(mut self, include_fields: &'a SearchFields) -> Self {
        self.include_fields = Some(include_fields.to_query_by_parameter());
        self
    }

    /// Excludes fields from the payload.
    #[must_use]
    pub fn with_exclude_fields(mut self, exclude_fields: &'a SearchFields) -> Self {
        self.exclude_fields = Some(exclude_fields.to_query_by_parameter());
        self
    }

    /// Explicitly sets highlighted fields.
    #[must_use]
    pub fn with_highlight_fields(mut self, highlight_fields: &'a SearchFields) -> Self {
        self.highlight_fields = Some(highlight_fields.to_query_by_parameter());
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
        collection: &'a CollectionName,
        q: &'a SearchQuery,
        query_by: &'a SearchFields,
        filter_by: Option<&'a SearchFilter>,
        sort_by: Option<&'a SortBy>,
        page: PageNumber,
        per_page: PageSize,
    ) -> Self {
        Self {
            collection: collection.as_str(),
            q: q.as_str(),
            query_by: query_by.to_query_by_parameter(),
            filter_by: filter_by.map(|value| Cow::Borrowed(value.to_filter_by_parameter())),
            sort_by: sort_by.map(SortBy::to_query_value),
            use_cache: true,
            cache_ttl: None,
            query_by_weights: None,
            prefix: true,
            page: page.get(),
            per_page: per_page.get(),
            include_fields: None,
            exclude_fields: None,
            highlight_fields: None,
            num_typos: None,
            drop_tokens_threshold: None,
        }
    }

    /// Converts a standard request into a merge-search request item.
    #[must_use]
    pub fn from_request(collection: &'a CollectionName, request: &'a SearchRequest<'a>) -> Self {
        Self {
            collection: collection.as_str(),
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
    /// Maximum number of searches in a single request.
    pub const MAX_SEARCHES: usize = 16;

    /// Creates a multi-search payload.
    pub fn new(searches: Vec<MultiSearchRequestItem<'a>>) -> TypesenseResult<Self> {
        if searches.is_empty() {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::EmptyMultiSearch,
            });
        }
        if searches.len() > Self::MAX_SEARCHES {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::TooManyMultiSearches,
            });
        }
        Ok(Self { searches })
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

impl MultiSearchResponse {
    /// Rejects omitted or failed sub-searches even when the HTTP batch was 200.
    pub fn validate(self, expected_searches: usize) -> TypesenseResult<Self> {
        if self.results.len() != expected_searches {
            return Err(TypesenseError::Transport {
                reason: crate::typesense::TypesenseTransportReason::InvalidResponseBody,
            });
        }
        for search in &self.results {
            if search.error_present {
                return Err(TypesenseError::Transport {
                    reason: crate::typesense::TypesenseTransportReason::InvalidResponseBody,
                });
            }
            if let Some(code) = search.code {
                let status =
                    reqwest::StatusCode::from_u16(code).map_err(|_| TypesenseError::Transport {
                        reason: crate::typesense::TypesenseTransportReason::InvalidResponseBody,
                    })?;
                if !status.is_success() {
                    return Err(crate::typesense::error::map_status(status));
                }
            }
            if search.page == 0 {
                // A successful search result has a one-based page. Empty
                // objects and unclassified error bodies must not become an
                // apparently valid empty search.
                return Err(TypesenseError::Transport {
                    reason: crate::typesense::TypesenseTransportReason::InvalidResponseBody,
                });
            }
        }
        Ok(self)
    }
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
    #[serde(
        rename = "error",
        default,
        deserialize_with = "error_field_was_present"
    )]
    error_present: bool,
}

fn error_field_was_present<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let _ = serde::de::IgnoredAny::deserialize(deserializer)?;
    Ok(true)
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

#[cfg(test)]
mod tests {
    use super::{MultiSearchRequest, MultiSearchRequestItem, MultiSearchResponse};
    use crate::typesense::{
        CollectionName, PageNumber, PageSize, SearchFieldName, SearchFields, SearchQuery,
        TypesenseError, TypesenseRequestReason, TypesenseTransportReason, TypesenseUpstreamReason,
    };

    #[test]
    fn multi_search_count_is_bounded_and_each_item_is_typed() {
        let collection = CollectionName::parse("documents").expect("valid collection");
        let query = SearchQuery::parse("example").expect("valid query");
        let field = SearchFieldName::parse("title").expect("valid field");
        let fields = SearchFields::new(vec![field]).expect("valid fields");
        let page_size = PageSize::parse(5, 10).expect("valid page size");
        assert!(matches!(
            MultiSearchRequest::new(Vec::new()),
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::EmptyMultiSearch
            })
        ));
        let searches = (0..=MultiSearchRequest::MAX_SEARCHES)
            .map(|_| {
                MultiSearchRequestItem::new(
                    &collection,
                    &query,
                    &fields,
                    None,
                    None,
                    PageNumber::FIRST,
                    page_size,
                )
            })
            .collect();
        assert!(matches!(
            MultiSearchRequest::new(searches),
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::TooManyMultiSearches
            })
        ));
    }

    #[test]
    fn multi_search_rejects_failed_or_missing_subsearches() {
        let failed: MultiSearchResponse =
            serde_json::from_str(r#"{"results":[{"code":403,"hits":[]}]}"#)
                .expect("valid wire response");
        assert!(matches!(
            failed.validate(1),
            Err(TypesenseError::Upstream {
                reason: TypesenseUpstreamReason::AuthorizationFailed,
                ..
            })
        ));
        let omitted: MultiSearchResponse =
            serde_json::from_str(r#"{"results":[]}"#).expect("valid wire response");
        assert!(matches!(
            omitted.validate(1),
            Err(TypesenseError::Transport {
                reason: TypesenseTransportReason::InvalidResponseBody
            })
        ));
        for body in [
            r#"{"results":[{"error":"upstream rejected query"}]}"#,
            r#"{"results":[{}]}"#,
        ] {
            let malformed: MultiSearchResponse =
                serde_json::from_str(body).expect("wire response should decode");
            assert!(matches!(
                malformed.validate(1),
                Err(TypesenseError::Transport {
                    reason: TypesenseTransportReason::InvalidResponseBody
                })
            ));
        }
    }
}
