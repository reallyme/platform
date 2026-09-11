// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use super::validate_static_key_label;
    use crate::keys::error::KeyLabelError;

    #[test]
    fn accepts_stable_key_labels() {
        assert!(validate_static_key_label("tenant-1.core").is_ok());
    }

    #[test]
    fn rejects_empty_key_labels() {
        assert!(matches!(
            validate_static_key_label(""),
            Err(KeyLabelError::Empty)
        ));
    }

    #[test]
    fn rejects_ambiguous_key_label_bytes() {
        assert!(matches!(
            validate_static_key_label("Tenant/One"),
            Err(KeyLabelError::InvalidByte)
        ));
    }
}
