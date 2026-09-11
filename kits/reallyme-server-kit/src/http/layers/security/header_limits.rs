// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::config::HttpHeaderLimitConfig;
use crate::observability::HttpRejectionReason;
use axum::http::HeaderMap;

pub(super) fn header_limits_rejection_reason(
    headers: &HeaderMap,
    limits: HttpHeaderLimitConfig,
) -> Option<HttpRejectionReason> {
    if headers.len() > limits.max_header_count().as_usize() {
        return Some(HttpRejectionReason::HeaderCountLimit);
    }

    let mut total_bytes = 0usize;
    for (name, value) in headers {
        let Some(header_bytes) = name
            .as_str()
            .len()
            .checked_add(value.as_bytes().len())
            .and_then(|bytes| bytes.checked_add(4))
        else {
            return Some(HttpRejectionReason::HeaderBytesLimit);
        };
        let Some(next_total_bytes) = total_bytes.checked_add(header_bytes) else {
            return Some(HttpRejectionReason::HeaderBytesLimit);
        };
        total_bytes = next_total_bytes;

        if total_bytes > limits.max_header_bytes().as_usize() {
            return Some(HttpRejectionReason::HeaderBytesLimit);
        }
    }

    None
}
