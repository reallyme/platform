// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{FilterValue, FiniteFilterFloat, SearchFilter, TextFilterValue};
use crate::typesense::{SearchFieldName, TypesenseError, TypesenseRequestReason, TypesenseResult};

#[test]
fn rejects_invalid_text_filter_value() {
    let value = TextFilterValue::parse("bad`value");

    assert!(matches!(
        value,
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::InvalidFilterValue
        })
    ));
}

#[test]
fn builds_exact_text_filter() -> TypesenseResult<()> {
    let field = SearchFieldName::parse("visibility")?;
    let value = TextFilterValue::parse("public")?;

    let filter = SearchFilter::exact(field, FilterValue::Text(value));

    assert_eq!(filter.to_filter_by_parameter(), "visibility:=`public`");
    Ok(())
}

#[test]
fn builds_inequality_range_and_geo_filters() -> TypesenseResult<()> {
    let excluded = SearchFilter::not_exact(
        SearchFieldName::parse("domain")?,
        FilterValue::Text(TextFilterValue::parse("example.com")?),
    );
    let recent =
        SearchFilter::greater_than_or_equal(SearchFieldName::parse("published_at")?, 1_700_000_000);
    let nearby = SearchFilter::within_radius_meters(
        SearchFieldName::parse("location")?,
        35.8989,
        14.5146,
        2_500,
    )?;

    let filter = excluded.and(recent)?.and(nearby)?;
    assert_eq!(
        filter.to_filter_by_parameter(),
        "((domain:!=`example.com`) && (published_at:>=1700000000)) && (location:(35.8989,14.5146,2.5 km))"
    );
    Ok(())
}

#[test]
fn rejects_non_finite_float_and_excessive_filter_depth() -> TypesenseResult<()> {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            FiniteFilterFloat::new(value),
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidFilterValue
            })
        ));
    }
    let field = SearchFieldName::parse("score")?;
    let finite = FiniteFilterFloat::new(1.5)?;
    let mut filter = SearchFilter::exact(field.clone(), FilterValue::Float(finite));
    for _ in 1..16 {
        filter = filter.and(SearchFilter::exact(field.clone(), FilterValue::Bool(true)))?;
    }
    assert!(matches!(
        filter.and(SearchFilter::exact(field, FilterValue::Bool(true))),
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::InvalidFilterValue
        })
    ));
    Ok(())
}

#[test]
fn rejects_invalid_geo_radius_values() -> TypesenseResult<()> {
    let field = SearchFieldName::parse("location")?;
    assert!(SearchFilter::within_radius_meters(field.clone(), 91.0, 0.0, 1).is_err());
    assert!(SearchFilter::within_radius_meters(field, 0.0, 0.0, 0).is_err());
    Ok(())
}
