// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{PageNumber, PageSize};
use crate::typesense::{TypesenseError, TypesenseRequestReason};

#[test]
fn accepts_page_size_within_bound() {
    let page_size = PageSize::parse(5, PageSize::MAX_SEARCH_AS_YOU_TYPE);

    assert!(matches!(page_size, Ok(value) if value.get() == 5));
}

#[test]
fn rejects_zero_page_size() {
    let page_size = PageSize::parse(0, PageSize::MAX_SEARCH_AS_YOU_TYPE);

    assert!(matches!(
        page_size,
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::ZeroPageSize
        })
    ));
}

#[test]
fn rejects_page_size_above_bound() {
    let page_size = PageSize::parse(11, PageSize::MAX_SEARCH_AS_YOU_TYPE);

    assert!(matches!(
        page_size,
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::PageSizeTooLarge
        })
    ));
}

#[test]
fn accepts_positive_page_number() {
    let page = PageNumber::parse(1);

    assert!(matches!(page, Ok(value) if value.get() == 1));
}

#[test]
fn rejects_zero_page_number() {
    let page = PageNumber::parse(0);

    assert!(matches!(
        page,
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::ZeroPageNumber
        })
    ));
}
