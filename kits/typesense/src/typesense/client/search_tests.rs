// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
