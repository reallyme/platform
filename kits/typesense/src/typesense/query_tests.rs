// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::SearchQuery;
use crate::typesense::{TypesenseError, TypesenseRequestReason};

#[test]
fn accepts_valid_handle_search_query() {
    let query = SearchQuery::parse("  @ali.ce  ");

    assert!(matches!(query, Ok(ref value) if value.as_str() == "@ali.ce"));
}

#[test]
fn accepts_utf8_and_punctuation() {
    let query = SearchQuery::parse("O'Connor & Sons 🌟");

    assert!(matches!(
        query,
        Ok(ref value) if value.as_str() == "O'Connor & Sons 🌟"
    ));
}

#[test]
fn rejects_empty_query() {
    let query = SearchQuery::parse("   ");

    assert!(matches!(
        query,
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::EmptyQuery
        })
    ));
}

#[test]
fn rejects_query_under_minimum_length() {
    let query = SearchQuery::parse("a");

    assert!(matches!(
        query,
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::QueryTooShort
        })
    ));
}

#[test]
fn rejects_query_over_maximum_length() {
    let query = SearchQuery::parse("a".repeat(65));

    assert!(matches!(
        query,
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::QueryTooLong
        })
    ));
}

#[test]
fn rejects_query_with_unsupported_characters() {
    let query = SearchQuery::parse("alice\u{0000}");

    assert!(matches!(
        query,
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::QueryContainsInvalidCharacters
        })
    ));
}
