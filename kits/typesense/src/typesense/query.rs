// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
#[path = "query_tests.rs"]
mod tests;
