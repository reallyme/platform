// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pagination types for Typesense requests.

use crate::typesense::{TypesenseError, TypesenseRequestReason, error::TypesenseResult};

/// One-based Typesense page number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageNumber(u32);

impl PageNumber {
    /// First page.
    pub const FIRST: Self = Self(1);

    /// Returns the page number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Parses a page number with one-based validation.
    pub fn parse(value: u32) -> TypesenseResult<Self> {
        if value == 0 {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::ZeroPageNumber,
            });
        }

        Ok(Self(value))
    }
}

/// Bounded per-page result count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSize(u8);

impl PageSize {
    /// Maximum page size for search-as-you-type usage.
    pub const MAX_SEARCH_AS_YOU_TYPE: u8 = 10;

    /// Parses a page size with an inclusive upper bound.
    pub fn parse(value: u8, max: u8) -> TypesenseResult<Self> {
        if value == 0 {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::ZeroPageSize,
            });
        }

        if value > max {
            return Err(TypesenseError::InvalidRequest {
                reason: TypesenseRequestReason::PageSizeTooLarge,
            });
        }

        Ok(Self(value))
    }

    /// Returns the page size.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
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
}
