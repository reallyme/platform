// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Sort models for Typesense requests.

use serde::{Deserialize, Serialize};

use crate::typesense::{
    SearchFieldName, TypesenseError, TypesenseRequestReason, error::TypesenseResult,
};

const MAX_SORT_ORDERS: usize = 4;

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    /// Ascending order.
    Asc,
    /// Descending order.
    Desc,
}

impl SortDirection {
    /// Returns the Typesense wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Sortable field name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortField(SearchFieldName);

impl SortField {
    /// Parses a sortable field name.
    pub fn parse(raw: impl Into<String>) -> TypesenseResult<Self> {
        SearchFieldName::parse(raw).map(Self)
    }

    /// Creates a sort field from an already-validated search field.
    #[must_use]
    pub const fn from_search_field(field: SearchFieldName) -> Self {
        Self(field)
    }

    /// Returns the field name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// One sort clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortOrder {
    field: SortField,
    direction: SortDirection,
    query_value: String,
}

impl SortOrder {
    /// Creates a sort order.
    #[must_use]
    pub fn new(field: SortField, direction: SortDirection) -> Self {
        let mut value = String::new();
        value.push_str(field.as_str());
        value.push(':');
        value.push_str(direction.as_str());
        Self {
            field,
            direction,
            query_value: value,
        }
    }

    /// Returns the Typesense `sort_by` clause for this order.
    #[must_use]
    pub fn to_query_value(&self) -> &str {
        self.query_value.as_str()
    }
}

/// Bounded list of sort orders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortBy {
    orders: Vec<SortOrder>,
    query_value: String,
}

impl SortBy {
    /// Maximum number of sort orders supported by Typesense.
    pub const MAX_ORDERS: usize = MAX_SORT_ORDERS;

    /// Creates a non-empty bounded sort clause.
    pub fn new(orders: Vec<SortOrder>) -> TypesenseResult<Self> {
        if orders.is_empty() {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::EmptySortOrders,
            });
        }

        if orders.len() > Self::MAX_ORDERS {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::TooManySortOrders,
            });
        }

        let query_value = orders
            .iter()
            .map(SortOrder::to_query_value)
            .collect::<Vec<_>>()
            .join(",");

        Ok(Self {
            orders,
            query_value,
        })
    }

    /// Returns the validated orders in this sort expression.
    #[must_use]
    pub fn orders(&self) -> &[SortOrder] {
        &self.orders
    }

    /// Returns the Typesense `sort_by` parameter value.
    #[must_use]
    pub fn to_query_value(&self) -> &str {
        self.query_value.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::{SortBy, SortDirection, SortField, SortOrder};
    use crate::typesense::{TypesenseError, TypesenseRequestReason, TypesenseResult};

    #[test]
    fn rejects_empty_sort_orders() {
        let sort_by = SortBy::new(Vec::new());

        assert!(matches!(
            sort_by,
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::EmptySortOrders
            })
        ));
    }

    #[test]
    fn rejects_sort_orders_above_bound() -> TypesenseResult<()> {
        let field = SortField::parse("rank_weight")?;
        let orders = vec![
            SortOrder::new(field.clone(), SortDirection::Desc),
            SortOrder::new(field.clone(), SortDirection::Desc),
            SortOrder::new(field.clone(), SortDirection::Desc),
            SortOrder::new(field.clone(), SortDirection::Desc),
            SortOrder::new(field, SortDirection::Desc),
        ];

        let sort_by = SortBy::new(orders);

        assert!(matches!(
            sort_by,
            Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::TooManySortOrders
            })
        ));
        Ok(())
    }
}
