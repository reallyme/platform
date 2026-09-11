// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed filter expression builders.

use crate::typesense::{
    SearchFieldName, TypesenseError, TypesenseRequestReason, error::TypesenseResult,
};

use std::sync::OnceLock;

const MIN_GEO_RADIUS_METERS: u32 = 1;
const MAX_GEO_RADIUS_METERS: u32 = 10_000_000;

/// Validated string literal for filter expressions.
#[derive(Clone, PartialEq, Eq)]
pub struct TextFilterValue(String);

impl TextFilterValue {
    /// Parses a string value for use in a Typesense filter.
    pub fn parse(raw: impl Into<String>) -> TypesenseResult<Self> {
        let value = raw.into();

        if value.is_empty()
            || value
                .chars()
                .any(|character| character == '`' || character.is_control())
        {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidFilterValue,
            });
        }

        Ok(Self(value))
    }

    fn append_typesense_literal(&self, output: &mut String) {
        output.push('`');
        output.push_str(&self.0);
        output.push('`');
    }
}

/// Typed value for an exact-match filter clause.
#[derive(Clone, PartialEq)]
pub enum FilterValue {
    /// Exact string match.
    Text(TextFilterValue),
    /// Boolean match.
    Bool(bool),
    /// 64-bit integer match.
    Int64(i64),
    /// 64-bit floating point match.
    Float(f64),
}

impl FilterValue {
    fn append_typesense_exact_clause(&self, output: &mut String) {
        match self {
            Self::Text(value) => {
                output.push_str(":=");
                value.append_typesense_literal(output);
            }
            Self::Bool(value) => {
                output.push(':');
                output.push_str(if *value { "true" } else { "false" });
            }
            Self::Int64(value) => {
                output.push(':');
                output.push_str(&value.to_string());
            }
            Self::Float(value) => {
                output.push(':');
                output.push_str(&value.to_string());
            }
        }
    }
}

#[derive(Clone, PartialEq)]
enum SearchFilterExpression {
    Exact {
        field: SearchFieldName,
        value: FilterValue,
    },
    NotExact {
        field: SearchFieldName,
        value: FilterValue,
    },
    Range {
        field: SearchFieldName,
        comparison: RangeComparison,
        value: i64,
    },
    GeoRadius {
        field: SearchFieldName,
        latitude: f64,
        longitude: f64,
        radius_meters: u32,
    },
    And {
        left: Box<SearchFilter>,
        right: Box<SearchFilter>,
    },
    Or {
        left: Box<SearchFilter>,
        right: Box<SearchFilter>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RangeComparison {
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
}

/// Prevalidated Typesense filter expression.
#[derive(Clone)]
pub struct SearchFilter {
    expression: SearchFilterExpression,
    filter_by_parameter: OnceLock<String>,
}

impl PartialEq for SearchFilter {
    fn eq(&self, other: &Self) -> bool {
        self.expression == other.expression
    }
}

impl Eq for SearchFilter {}

impl SearchFilter {
    fn new(expression: SearchFilterExpression) -> Self {
        Self {
            expression,
            filter_by_parameter: OnceLock::new(),
        }
    }

    fn write_expression(expression: &SearchFilterExpression, output: &mut String) {
        match expression {
            SearchFilterExpression::Exact { field, value } => {
                output.push_str(field.as_str());
                value.append_typesense_exact_clause(output);
            }
            SearchFilterExpression::NotExact { field, value } => {
                output.push_str(field.as_str());
                output.push_str(":!=");
                match value {
                    FilterValue::Text(value) => value.append_typesense_literal(output),
                    FilterValue::Bool(value) => {
                        output.push_str(if *value { "true" } else { "false" });
                    }
                    FilterValue::Int64(value) => output.push_str(&value.to_string()),
                    FilterValue::Float(value) => output.push_str(&value.to_string()),
                }
            }
            SearchFilterExpression::Range {
                field,
                comparison,
                value,
            } => {
                output.push_str(field.as_str());
                output.push(':');
                output.push_str(match comparison {
                    RangeComparison::GreaterThanOrEqual => ">=",
                    RangeComparison::LessThan => "<",
                    RangeComparison::LessThanOrEqual => "<=",
                });
                output.push_str(&value.to_string());
            }
            SearchFilterExpression::GeoRadius {
                field,
                latitude,
                longitude,
                radius_meters,
            } => {
                output.push_str(field.as_str());
                output.push_str(":(");
                output.push_str(&latitude.to_string());
                output.push(',');
                output.push_str(&longitude.to_string());
                output.push(',');
                // Typesense documents kilometres as the portable radius unit.
                // Keeping metres in the public constructor avoids unit mistakes
                // at call sites while preserving sub-kilometre precision.
                output.push_str(&(f64::from(*radius_meters) / 1_000.0).to_string());
                output.push_str(" km)");
            }
            SearchFilterExpression::And { left, right } => {
                output.push('(');
                Self::write_expression(&left.expression, output);
                output.push_str(") && (");
                Self::write_expression(&right.expression, output);
                output.push(')');
            }
            SearchFilterExpression::Or { left, right } => {
                output.push('(');
                Self::write_expression(&left.expression, output);
                output.push_str(") || (");
                Self::write_expression(&right.expression, output);
                output.push(')');
            }
        }
    }

    fn append_to_string(&self, output: &mut String) {
        Self::write_expression(&self.expression, output)
    }

    /// Returns the Typesense wire value.
    pub fn to_filter_by_parameter(&self) -> &str {
        self.filter_by_parameter
            .get_or_init(|| {
                let mut filter_by_parameter = String::new();
                self.append_to_string(&mut filter_by_parameter);

                filter_by_parameter
            })
            .as_str()
    }

    /// Returns true when filter cache has already been emitted.
    #[must_use]
    pub fn has_cached_filter_by_parameter(&self) -> bool {
        self.filter_by_parameter.get().is_some()
    }
}

impl SearchFilter {
    /// Creates an exact-match filter clause from typed inputs.
    #[must_use]
    pub fn exact(field: SearchFieldName, value: FilterValue) -> Self {
        Self::new(SearchFilterExpression::Exact { field, value })
    }

    /// Creates an exact inequality clause from typed inputs.
    #[must_use]
    pub fn not_exact(field: SearchFieldName, value: FilterValue) -> Self {
        Self::new(SearchFilterExpression::NotExact { field, value })
    }

    /// Creates an inclusive lower bound for a signed integer field.
    #[must_use]
    pub fn greater_than_or_equal(field: SearchFieldName, value: i64) -> Self {
        Self::new(SearchFilterExpression::Range {
            field,
            comparison: RangeComparison::GreaterThanOrEqual,
            value,
        })
    }

    /// Creates an exclusive upper bound for a signed integer field.
    #[must_use]
    pub fn less_than(field: SearchFieldName, value: i64) -> Self {
        Self::new(SearchFilterExpression::Range {
            field,
            comparison: RangeComparison::LessThan,
            value,
        })
    }

    /// Creates an inclusive upper bound for a signed integer field.
    #[must_use]
    pub fn less_than_or_equal(field: SearchFieldName, value: i64) -> Self {
        Self::new(SearchFilterExpression::Range {
            field,
            comparison: RangeComparison::LessThanOrEqual,
            value,
        })
    }

    /// Creates a bounded geospatial radius clause.
    pub fn within_radius_meters(
        field: SearchFieldName,
        latitude: f64,
        longitude: f64,
        radius_meters: u32,
    ) -> TypesenseResult<Self> {
        if !latitude.is_finite()
            || !longitude.is_finite()
            || !(-90.0..=90.0).contains(&latitude)
            || !(-180.0..=180.0).contains(&longitude)
            || !(MIN_GEO_RADIUS_METERS..=MAX_GEO_RADIUS_METERS).contains(&radius_meters)
        {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::InvalidFilterValue,
            });
        }
        Ok(Self::new(SearchFilterExpression::GeoRadius {
            field,
            latitude,
            longitude,
            radius_meters,
        }))
    }

    /// Combines two filters with `&&`.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self::new(SearchFilterExpression::And {
            left: Box::new(self),
            right: Box::new(other),
        })
    }

    /// Combines two filters with `||`.
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self::new(SearchFilterExpression::Or {
            left: Box::new(self),
            right: Box::new(other),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FilterValue, SearchFilter, TextFilterValue};
    use crate::typesense::{
        SearchFieldName, TypesenseError, TypesenseRequestReason, TypesenseResult,
    };

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
        let recent = SearchFilter::greater_than_or_equal(
            SearchFieldName::parse("published_at")?,
            1_700_000_000,
        );
        let nearby = SearchFilter::within_radius_meters(
            SearchFieldName::parse("location")?,
            35.8989,
            14.5146,
            2_500,
        )?;

        let filter = excluded.and(recent).and(nearby);
        assert_eq!(
            filter.to_filter_by_parameter(),
            "((domain:!=`example.com`) && (published_at:>=1700000000)) && (location:(35.8989,14.5146,2.5 km))"
        );
        Ok(())
    }

    #[test]
    fn rejects_invalid_geo_radius_values() -> TypesenseResult<()> {
        let field = SearchFieldName::parse("location")?;
        assert!(SearchFilter::within_radius_meters(field.clone(), 91.0, 0.0, 1).is_err());
        assert!(SearchFilter::within_radius_meters(field, 0.0, 0.0, 0).is_err());
        Ok(())
    }
}
