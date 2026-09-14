// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
