// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

pub(crate) struct NameValidation {
    separators: &'static [u8],
    boundaries: &'static [u8],
    max_len: usize,
    disallow_consecutive_separators: bool,
}

impl NameValidation {
    pub(crate) const fn new(
        separators: &'static [u8],
        boundaries: &'static [u8],
        max_len: usize,
        disallow_consecutive_separators: bool,
    ) -> Self {
        Self {
            separators,
            boundaries,
            max_len,
            disallow_consecutive_separators,
        }
    }
}

pub(crate) fn validate_name(
    value: &str,
    field: AppKitField,
    rules: &NameValidation,
) -> Result<(), AppKitError> {
    if value.is_empty() {
        return Err(AppKitError::new(field, AppKitErrorReason::Empty));
    }

    if value.len() > rules.max_len {
        return Err(AppKitError::new(field, AppKitErrorReason::TooLong));
    }

    let first = value
        .as_bytes()
        .first()
        .ok_or_else(|| AppKitError::new(field, AppKitErrorReason::Empty))?;
    let last = value
        .as_bytes()
        .last()
        .ok_or_else(|| AppKitError::new(field, AppKitErrorReason::Empty))?;

    if rules.boundaries.contains(first) || rules.boundaries.contains(last) {
        return Err(AppKitError::new(field, AppKitErrorReason::InvalidBoundary));
    }

    let mut prev_was_separator = false;

    for byte in value.bytes() {
        let is_separator = rules.separators.contains(&byte);
        if is_separator {
            if rules.disallow_consecutive_separators && prev_was_separator {
                return Err(AppKitError::new(field, AppKitErrorReason::InvalidCharacter));
            }
            prev_was_separator = true;
            continue;
        }

        if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() {
            return Err(AppKitError::new(field, AppKitErrorReason::InvalidCharacter));
        }

        prev_was_separator = false;
    }

    Ok(())
}
