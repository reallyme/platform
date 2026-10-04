// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Standard security headers applied to every response.

use axum::http::{HeaderName, HeaderValue};
use axum::response::Response;

use crate::config::SecurityHeadersConfig;

pub(super) const X_CONTENT_TYPE_OPTIONS_HEADER: HeaderName =
    HeaderName::from_static("x-content-type-options");
pub(super) const REFERRER_POLICY_HEADER: HeaderName = HeaderName::from_static("referrer-policy");
pub(super) const X_FRAME_OPTIONS_HEADER: HeaderName = HeaderName::from_static("x-frame-options");
pub(super) const CONTENT_SECURITY_POLICY_HEADER: HeaderName =
    HeaderName::from_static("content-security-policy");
const CROSS_ORIGIN_RESOURCE_POLICY_HEADER: HeaderName =
    HeaderName::from_static("cross-origin-resource-policy");
pub(super) const CACHE_CONTROL_HEADER: HeaderName = HeaderName::from_static("cache-control");

const NOSNIFF_VALUE: HeaderValue = HeaderValue::from_static("nosniff");
const REFERRER_POLICY_VALUE: HeaderValue =
    HeaderValue::from_static("strict-origin-when-cross-origin");
const DENY_VALUE: HeaderValue = HeaderValue::from_static("DENY");
const FRAME_ANCESTORS_NONE_VALUE: HeaderValue = HeaderValue::from_static("frame-ancestors 'none'");
const SAME_SITE_VALUE: HeaderValue = HeaderValue::from_static("same-site");
const NO_STORE_VALUE: HeaderValue = HeaderValue::from_static("no-store");

pub(super) fn apply_security_headers(response: &mut Response, config: SecurityHeadersConfig) {
    if !config.enabled() {
        return;
    }

    let should_disable_error_caching =
        response.status().is_client_error() || response.status().is_server_error();
    let headers = response.headers_mut();
    headers
        .entry(X_CONTENT_TYPE_OPTIONS_HEADER)
        .or_insert(NOSNIFF_VALUE);
    headers
        .entry(REFERRER_POLICY_HEADER)
        .or_insert(REFERRER_POLICY_VALUE);
    headers.entry(X_FRAME_OPTIONS_HEADER).or_insert(DENY_VALUE);
    headers
        .entry(CONTENT_SECURITY_POLICY_HEADER)
        .or_insert(FRAME_ANCESTORS_NONE_VALUE);
    headers
        .entry(CROSS_ORIGIN_RESOURCE_POLICY_HEADER)
        .or_insert(SAME_SITE_VALUE);

    if should_disable_error_caching {
        headers
            .entry(CACHE_CONTROL_HEADER)
            .or_insert(NO_STORE_VALUE);
    }
}
