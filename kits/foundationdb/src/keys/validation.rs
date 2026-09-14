// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared validation for stable key labels.

use crate::keys::error::KeyLabelError;

const MAX_KEY_LABEL_BYTES: usize = 64;

pub(crate) fn validate_static_key_label(value: &'static str) -> Result<(), KeyLabelError> {
    if value.is_empty() {
        return Err(KeyLabelError::Empty);
    }

    if value.len() > MAX_KEY_LABEL_BYTES {
        return Err(KeyLabelError::TooLong);
    }

    for byte in value.as_bytes() {
        let valid = byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(*byte, b'_' | b'-' | b'.');
        if !valid {
            return Err(KeyLabelError::InvalidByte);
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
