// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use serde_json::Value;

    use super::{CollectionField, CollectionName, CollectionSchema};
    use crate::typesense::{TypesenseError, TypesenseRequestReason};

    #[test]
    fn accepts_valid_collection_name() {
        let name = CollectionName::parse("people_directory");

        assert!(matches!(name, Ok(ref value) if value.as_str() == "people_directory"));
    }

    #[test]
    fn rejects_empty_collection_name() {
        let name = CollectionName::parse(" ");

        assert!(matches!(
            name,
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidCollectionName
            })
        ));
    }

    #[test]
    fn rejects_collection_name_with_path_separators() {
        let name = CollectionName::parse("../people");

        assert!(matches!(
            name,
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidCollectionName
            })
        ));
    }

    #[test]
    fn omits_alias_when_serializing_collection_schema() -> Result<(), TypesenseError> {
        let schema = CollectionSchema::new(
            CollectionName::parse("people_directory")?,
            vec![CollectionField::new(
                crate::typesense::SearchFieldName::parse("handle")?,
                crate::typesense::FieldKind::String,
            )],
            None,
        )?
        .with_alias(CollectionName::parse("directory")?);

        let encoded =
            serde_json::to_value(&schema).map_err(|_| TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidFilterValue,
            })?;

        assert!(matches!(encoded, Value::Object(ref object) if !object.contains_key("alias")));
        Ok(())
    }

    #[test]
    fn omits_false_sort_to_preserve_type_specific_server_defaults() -> Result<(), TypesenseError> {
        let geopoint = CollectionField::new(
            crate::typesense::SearchFieldName::parse("location")?,
            crate::typesense::FieldKind::Geopoint,
        );
        let sortable = CollectionField::new(
            crate::typesense::SearchFieldName::parse("display_name")?,
            crate::typesense::FieldKind::String,
        )
        .sortable();

        let geopoint_json =
            serde_json::to_value(geopoint).map_err(|_| TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidFilterValue,
            })?;
        let sortable_json =
            serde_json::to_value(sortable).map_err(|_| TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidFilterValue,
            })?;

        assert!(matches!(geopoint_json, Value::Object(ref object) if !object.contains_key("sort")));
        assert!(
            matches!(sortable_json, Value::Object(ref object) if object.get("sort") == Some(&Value::Bool(true)))
        );
        Ok(())
    }

    #[test]
    fn deserializes_collection_schema_without_alias_field() -> Result<(), TypesenseError> {
        let decoded: CollectionSchema = serde_json::from_str(
            r#"{
                "name":"people_directory",
                "fields":[{"name":"handle","type":"string","optional":false,"facet":false,"sort":false}],
                "default_sorting_field":null
            }"#,
        )
        .map_err(|_| TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::InvalidFilterValue,
        })?;

        assert_eq!(decoded.alias, None);
        Ok(())
    }

    #[test]
    fn serializes_bounded_vector_dimensions() -> Result<(), TypesenseError> {
        let field = CollectionField::new(
            crate::typesense::SearchFieldName::parse("embedding")?,
            crate::typesense::FieldKind::FloatArray,
        )
        .with_vector_dimensions(384)?;
        let encoded = serde_json::to_value(field).map_err(|_| TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::InvalidFilterValue,
        })?;

        assert!(
            matches!(encoded, Value::Object(ref object) if object.get("num_dim") == Some(&Value::from(384)))
        );
        Ok(())
    }

    #[test]
    fn rejects_dimensions_on_non_vector_field() -> Result<(), TypesenseError> {
        let result = CollectionField::new(
            crate::typesense::SearchFieldName::parse("title")?,
            crate::typesense::FieldKind::String,
        )
        .with_vector_dimensions(384);

        assert!(matches!(
            result,
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidVectorDimensions
            })
        ));
        Ok(())
    }
}

impl TryFrom<String> for CollectionName {
    type Error = TypesenseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod deserialization_tests {
    use super::CollectionName;
    #[test]
    fn deserialization_enforces_constructor_validation() {
        for raw in ["../other", "name/../../keys", "field:!=secret", "", "a\\b"] {
            let json = serde_json::to_string(raw).expect("JSON string");
            assert!(serde_json::from_str::<CollectionName>(&json).is_err());
        }
        let parsed: CollectionName =
            serde_json::from_str("\"safe_name-1\"").expect("valid identifier");
        assert_eq!(parsed.as_str(), "safe_name-1");
        assert_eq!(
            serde_json::to_string(&parsed).expect("serialization"),
            "\"safe_name-1\""
        );
    }
}
