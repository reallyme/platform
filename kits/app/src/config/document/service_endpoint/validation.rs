// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{AppConfigDocumentError, AppConfigDocumentErrorReason, MAX_IDENTIFIER_BYTES};

pub(super) fn validate_identifier(value: &str) -> Result<(), AppConfigDocumentError> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InvalidLocatorIdentifier,
        ));
    }

    if value.bytes().any(|byte| {
        !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_')
    }) {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InvalidLocatorIdentifier,
        ));
    }

    Ok(())
}
