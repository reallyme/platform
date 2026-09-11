// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed search document primitives.

use serde::{Deserialize, Serialize};

use crate::typesense::{TypesenseError, TypesenseRequestReason, error::TypesenseResult};

/// Validated Typesense document id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct DocumentId(String);

impl DocumentId {
    /// Parses a document id.
    pub fn parse(raw: impl Into<String>) -> TypesenseResult<Self> {
        let value = raw.into();
        let trimmed = value.trim();
        let valid = !trimmed.is_empty()
            && !matches!(trimmed, "." | "..")
            && trimmed.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ':' | '.')
            });

        if !valid {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidDocumentId,
            });
        }

        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Trait implemented by documents that can be written to a Typesense collection.
///
/// This trait is intended for static dispatch in generic indexing helpers. It is
/// not designed as a trait-object API.
pub trait SearchDocument: Serialize + Send + Sync {
    /// Returns the document id used by Typesense.
    fn document_id(&self) -> &DocumentId;
}

impl TryFrom<String> for DocumentId {
    type Error = TypesenseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod deserialization_tests {
    use super::DocumentId;
    #[test]
    fn deserialization_enforces_constructor_validation() {
        for raw in [
            ".",
            "..",
            "../other",
            "name/../../keys",
            "field:!=secret",
            "",
            "a\\b",
        ] {
            let json = serde_json::to_string(raw).expect("JSON string");
            assert!(serde_json::from_str::<DocumentId>(&json).is_err());
        }
        let parsed: DocumentId = serde_json::from_str("\"safe_name-1\"").expect("valid identifier");
        assert_eq!(parsed.as_str(), "safe_name-1");
        assert_eq!(
            serde_json::to_string(&parsed).expect("serialization"),
            "\"safe_name-1\""
        );
    }
}
