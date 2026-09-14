// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
