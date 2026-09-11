// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Search query validation and normalization.

use crate::typesense::{TypesenseError, TypesenseRequestReason, error::TypesenseResult};

const MIN_QUERY_LENGTH: usize = 2;
const MAX_QUERY_LENGTH: usize = 64;

/// Validated search-as-you-type query.
#[derive(Clone, PartialEq, Eq)]
pub struct SearchQuery(String);

impl SearchQuery {
    /// Parses and normalizes user search text.
    pub fn parse(raw: impl AsRef<str>) -> TypesenseResult<Self> {
        let trimmed = raw.as_ref().trim();

        if trimmed.is_empty() {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::EmptyQuery,
            });
        }

        let normalized = trimmed;

        let length = normalized.chars().count();
        if length < MIN_QUERY_LENGTH {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::QueryTooShort,
            });
        }

        if length > MAX_QUERY_LENGTH {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::QueryTooLong,
            });
        }

        if normalized.chars().any(char::is_control) {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::QueryContainsInvalidCharacters,
            });
        }

        Ok(Self(normalized.to_owned()))
    }

    /// Returns the query value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
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
}
