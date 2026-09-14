// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed field identifiers used in search queries, filters, and sort clauses.

use serde::{Deserialize, Serialize};

use crate::typesense::{TypesenseError, TypesenseRequestReason, error::TypesenseResult};

const MAX_QUERY_FIELDS: usize = 8;

/// Validated field identifier for search query parameters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct SearchFieldName(String);

impl SearchFieldName {
    /// Parses a field name that is safe to use in search, filter, and sort clauses.
    pub fn parse(raw: impl Into<String>) -> TypesenseResult<Self> {
        let value = raw.into();
        let trimmed = value.trim();
        let valid = !trimmed.is_empty()
            && trimmed.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
            });

        if !valid {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidSearchFieldName,
            });
        }

        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated field name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Non-empty bounded list of search fields used for `query_by`.
#[derive(Clone, PartialEq, Eq)]
pub struct SearchFields {
    fields: Vec<SearchFieldName>,
    query_by_parameter: String,
}

impl SearchFields {
    /// Maximum number of query fields supported by the kit.
    pub const MAX_FIELDS: usize = MAX_QUERY_FIELDS;

    /// Creates a bounded non-empty list of query fields.
    pub fn new(fields: Vec<SearchFieldName>) -> TypesenseResult<Self> {
        if fields.is_empty() {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::EmptyQueryFields,
            });
        }

        if fields.len() > Self::MAX_FIELDS {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::TooManyQueryFields,
            });
        }

        let query_by_parameter = fields
            .iter()
            .map(SearchFieldName::as_str)
            .collect::<Vec<_>>()
            .join(",");

        Ok(Self {
            fields,
            query_by_parameter,
        })
    }

    /// Returns the validated field list.
    #[must_use]
    pub fn fields(&self) -> &[SearchFieldName] {
        &self.fields
    }

    /// Returns the Typesense `query_by` parameter value.
    #[must_use]
    pub fn to_query_by_parameter(&self) -> &str {
        self.query_by_parameter.as_str()
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;

impl TryFrom<String> for SearchFieldName {
    type Error = TypesenseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(test)]
#[path = "field_deserialization_tests.rs"]
mod deserialization_tests;
