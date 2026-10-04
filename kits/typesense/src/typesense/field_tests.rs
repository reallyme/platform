// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{SearchFieldName, SearchFields, SearchQueryWeights};
use crate::typesense::{TypesenseError, TypesenseRequestReason, TypesenseResult};
use std::num::NonZeroU8;

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

#[test]
fn query_weights_require_one_positive_weight_per_field() -> TypesenseResult<()> {
    let fields = SearchFields::new(vec![
        SearchFieldName::parse("name")?,
        SearchFieldName::parse("description")?,
    ])?;
    let first = NonZeroU8::new(3).expect("positive test weight");
    let second = NonZeroU8::new(1).expect("positive test weight");
    let weights = SearchQueryWeights::new(&fields, vec![first, second])?;
    assert_eq!(weights.as_str(), "3,1");

    assert!(matches!(
        SearchQueryWeights::new(&fields, vec![first]),
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::InvalidQueryWeights
        })
    ));
    Ok(())
}
