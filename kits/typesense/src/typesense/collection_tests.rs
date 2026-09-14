// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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

    let encoded = serde_json::to_value(&schema).map_err(|_| TypesenseError::InvalidRequest {
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
