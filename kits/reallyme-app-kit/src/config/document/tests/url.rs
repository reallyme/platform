// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::super::{
    AppBaseUrl, AppConfigDocumentError, AppConfigDocumentErrorReason, AppDownstreamBaseUrl,
};

#[test]
fn app_base_url_accepts_valid_production_https_origin() {
    let value = AppBaseUrl::new("https://api.reallyme.net").expect("valid production HTTPS origin");

    assert_eq!(value.as_str(), "https://api.reallyme.net");
}

#[test]
fn app_base_url_accepts_valid_localhost_http_origin() {
    let value = AppBaseUrl::new("http://localhost:3000").expect("valid local HTTP origin");

    assert_eq!(value.as_str(), "http://localhost:3000");
}

#[test]
fn app_base_url_accepts_loopback_ipv6_literal_forms() {
    let canonical = AppBaseUrl::new("http://[::1]:3000").expect("valid loopback ipv6 literal");
    let expanded = AppBaseUrl::new("http://[0:0:0:0:0:0:0:1]:3000")
        .expect("valid expanded loopback ipv6 literal");

    assert_eq!(canonical.as_str(), "http://[::1]:3000");
    assert_eq!(expanded.as_str(), "http://[0:0:0:0:0:0:0:1]:3000");
}

#[test]
fn app_base_url_rejects_non_local_http_with_ipv6_authority_prefix_match_only() {
    assert_url_invalid(
        "http://[::1]foobar:3000",
        AppConfigDocumentErrorReason::InvalidUrlPort,
    );
}

#[test]
fn app_base_url_accepts_ipv4_loopback_range_alias() {
    let value = AppBaseUrl::new("http://127.0.0.2:3000").expect("valid ipv4 loopback alias");

    assert_eq!(value.as_str(), "http://127.0.0.2:3000");
}

#[test]
fn app_base_url_rejects_unsafe_shapes() {
    assert_url_invalid("", AppConfigDocumentErrorReason::EmptyUrl);
    assert_url_invalid(
        "http://api.reallyme.net",
        AppConfigDocumentErrorReason::InsecureNonLocalHttpOrigin,
    );
    assert_url_invalid(
        "https://api.reallyme.net value",
        AppConfigDocumentErrorReason::ContainsWhitespace,
    );
    assert_url_invalid(
        "https://api.reallyme.net?debug=true",
        AppConfigDocumentErrorReason::ContainsQuery,
    );
    assert_url_invalid(
        "https://api.reallyme.net#fragment",
        AppConfigDocumentErrorReason::ContainsFragment,
    );
    assert_url_invalid(
        "https://api.reallyme.net/v1",
        AppConfigDocumentErrorReason::ContainsPath,
    );
    assert_url_invalid(
        "https://token@api.reallyme.net",
        AppConfigDocumentErrorReason::ContainsUserInfo,
    );
    assert_url_invalid(
        "api.reallyme.net",
        AppConfigDocumentErrorReason::InvalidUrlScheme,
    );
    assert_url_invalid("https://", AppConfigDocumentErrorReason::MissingUrlHost);
    assert_url_invalid(
        "http://localhost:99999",
        AppConfigDocumentErrorReason::InvalidUrlPort,
    );
    assert_url_invalid(
        "http://localhost:abc",
        AppConfigDocumentErrorReason::InvalidUrlPort,
    );
}

#[test]
fn downstream_base_url_rejects_non_local_plain_http() {
    assert_eq!(
        AppDownstreamBaseUrl::new("http://api.reallyme.net")
            .map_err(AppConfigDocumentError::reason),
        Err(AppConfigDocumentErrorReason::InsecureNonLocalHttpOrigin),
    );
}

fn assert_url_invalid(value: &str, expected: AppConfigDocumentErrorReason) {
    assert_eq!(
        AppBaseUrl::new(value).map_err(AppConfigDocumentError::reason),
        Err(expected),
    );
}
