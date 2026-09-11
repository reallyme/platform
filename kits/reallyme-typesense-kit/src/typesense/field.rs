// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use super::{SearchFieldName, SearchFields};
    use crate::typesense::{TypesenseError, TypesenseRequestReason, TypesenseResult};

    #[test]
    fn accepts_safe_field_name() {
        let field = SearchFieldName::parse("profile.handle");

        assert!(matches!(field, Ok(ref value) if value.as_str() == "profile.handle"));
    }

    #[test]
    fn rejects_invalid_field_name() {
        let field = SearchFieldName::parse("profile(handle)");

        assert!(matches!(
            field,
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidSearchFieldName
            })
        ));
    }

    #[test]
    fn rejects_empty_query_field_list() {
        let fields = SearchFields::new(Vec::new());

        assert!(matches!(
            fields,
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::EmptyQueryFields
            })
        ));
    }

    #[test]
    fn rejects_query_field_list_above_bound() -> TypesenseResult<()> {
        let fields = (0..=SearchFields::MAX_FIELDS)
            .map(|index| SearchFieldName::parse(format!("field_{index}")))
            .collect::<TypesenseResult<Vec<_>>>()?;

        let fields = SearchFields::new(fields);

        assert!(matches!(
            fields,
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::TooManyQueryFields
            })
        ));
        Ok(())
    }
}

impl TryFrom<String> for SearchFieldName {
    type Error = TypesenseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod deserialization_tests {
    use super::SearchFieldName;
    #[test]
    fn deserialization_enforces_constructor_validation() {
        for raw in ["../other", "name/../../keys", "field:!=secret", "", "a\\b"] {
            let json = serde_json::to_string(raw).expect("JSON string");
            assert!(serde_json::from_str::<SearchFieldName>(&json).is_err());
        }
        let parsed: SearchFieldName =
            serde_json::from_str("\"safe_name-1\"").expect("valid identifier");
        assert_eq!(parsed.as_str(), "safe_name-1");
        assert_eq!(
            serde_json::to_string(&parsed).expect("serialization"),
            "\"safe_name-1\""
        );
    }
}
