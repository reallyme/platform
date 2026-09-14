// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::HttpRoutePolicyErrorReason;

pub(super) const MAX_HTTP_LISTENER_NAME_BYTES: usize = 63;
pub(super) const MAX_HTTP_VISIBILITY_CLASS_BYTES: usize = 63;
pub(super) const MAX_HTTP_ROUTE_PREFIX_BYTES: usize = 256;

pub(super) fn validate_symbolic_name(
    value: &str,
    max_bytes: usize,
) -> Result<(), HttpRoutePolicyErrorReason> {
    if value.is_empty() {
        return Err(HttpRoutePolicyErrorReason::Empty);
    }

    if value.len() > max_bytes {
        return Err(HttpRoutePolicyErrorReason::TooLong);
    }

    if value.starts_with('-') || value.ends_with('-') {
        return Err(HttpRoutePolicyErrorReason::InvalidBoundary);
    }

    if value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Ok(());
    }

    Err(HttpRoutePolicyErrorReason::InvalidCharacters)
}
