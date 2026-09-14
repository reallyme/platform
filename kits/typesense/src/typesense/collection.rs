// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Collection names and schema request models.

use serde::{Deserialize, Serialize};

use crate::typesense::{
    SearchFieldName, SortField, TypesenseError, TypesenseRequestReason, error::TypesenseResult,
    schema::FieldKind,
};

/// Validated Typesense collection name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct CollectionName(String);

impl CollectionName {
    /// Parses a collection name accepted by this kit.
    pub fn parse(raw: impl Into<String>) -> TypesenseResult<Self> {
        let value = raw.into();
        let trimmed = value.trim();
        let valid = !trimmed.is_empty()
            && trimmed.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            });

        if !valid {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidCollectionName,
            });
        }

        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the collection name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Typesense collection schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionSchema {
    /// Collection name.
    pub name: CollectionName,
    /// Field definitions.
    pub fields: Vec<CollectionField>,
    /// Optional default sorting field.
    pub default_sorting_field: Option<SortField>,
    /// Optional logical alias name that should point to this physical collection.
    #[serde(skip_serializing, default)]
    pub alias: Option<CollectionName>,
}

impl CollectionSchema {
    /// Creates a collection schema with at least one field.
    pub fn new(
        name: CollectionName,
        fields: Vec<CollectionField>,
        default_sorting_field: Option<SortField>,
    ) -> TypesenseResult<Self> {
        if fields.is_empty() {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::EmptyCollectionFields,
            });
        }

        Ok(Self {
            name,
            fields,
            default_sorting_field,
            alias: None,
        })
    }

    /// Attaches a logical alias for this physical collection schema.
    #[must_use]
    pub fn with_alias(mut self, alias: CollectionName) -> Self {
        self.alias = Some(alias);
        self
    }
}

/// Typesense collection field definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionField {
    /// Field name.
    pub name: SearchFieldName,
    /// Field type.
    #[serde(rename = "type")]
    pub kind: FieldKind,
    /// Whether the field is optional.
    #[serde(default)]
    pub optional: bool,
    /// Whether the field is indexed for faceting.
    #[serde(default)]
    pub facet: bool,
    /// Whether the field may be used in sorting.
    // Typesense requires geopoints to retain their implicit sort index and
    // rejects an explicit `sort: false`. Omitting false preserves the server's
    // type-specific defaults while still allowing callers to opt string fields
    // into sorting explicitly.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sort: bool,
    /// Fixed vector dimension count required by Typesense for float arrays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_dim: Option<u32>,
}

const fn is_false(value: &bool) -> bool {
    !*value
}

impl CollectionField {
    /// Creates a field definition with conservative defaults.
    #[must_use]
    pub fn new(name: SearchFieldName, kind: FieldKind) -> Self {
        Self {
            name,
            kind,
            optional: false,
            facet: false,
            sort: false,
            num_dim: None,
        }
    }

    /// Marks a field as optional.
    #[must_use]
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Marks a field as a facet field.
    #[must_use]
    pub fn faceted(mut self) -> Self {
        self.facet = true;
        self
    }

    /// Marks a field as sortable.
    #[must_use]
    pub fn sortable(mut self) -> Self {
        self.sort = true;
        self
    }

    /// Marks a float-array field as a fixed-dimension vector.
    ///
    /// Typesense schemas bind a vector field to one dimension count. Model
    /// migrations therefore require a new field or collection generation
    /// rather than silently reinterpreting stored vectors.
    pub fn with_vector_dimensions(mut self, dimensions: u32) -> TypesenseResult<Self> {
        const MAX_VECTOR_DIMENSIONS: u32 = 4_096;
        if self.kind != FieldKind::FloatArray
            || dimensions == 0
            || dimensions > MAX_VECTOR_DIMENSIONS
        {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidVectorDimensions,
            });
        }
        self.num_dim = Some(dimensions);
        Ok(self)
    }
}

#[cfg(test)]
#[path = "collection_tests.rs"]
mod tests;

impl TryFrom<String> for CollectionName {
    type Error = TypesenseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(test)]
#[path = "collection_deserialization_tests.rs"]
mod deserialization_tests;
