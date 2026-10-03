// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::{BoundedHttpsRequest, CapturedResponseHeader, HttpsExchangeLimits, HttpsMethod};
use crate::{HttpsDispatchOutcome, HttpsTransportErrorReason};

#[test]
fn accepts_a_bounded_request_and_protocol_headers() {
    let limits = HttpsExchangeLimits::new(128, 512).expect("valid limits");
    let captured = CapturedResponseHeader::new("x-jku-url", 256).expect("valid header");
    let request = BoundedHttpsRequest::new(
        HttpsMethod::Post,
        "/wallet_rp/create",
        br#"{"ok":true}"#,
        Duration::from_secs(5),
        limits,
    )
    .expect("valid request")
    .with_content_type("application/json")
    .expect("valid content type")
    .with_accept("application/jwt")
    .expect("valid accept")
    .with_captured_response_header(captured);

    assert_eq!(request.method, HttpsMethod::Post);
    assert_eq!(request.captured_response_header, Some(captured));
}

#[test]
fn rejects_zero_excessive_and_oversized_request_limits() {
    for invalid in [
        HttpsExchangeLimits::new(0, 1),
        HttpsExchangeLimits::new(1, 0),
        HttpsExchangeLimits::new(HttpsExchangeLimits::MAXIMUM_REQUEST_BYTES + 1, 1),
        HttpsExchangeLimits::new(1, HttpsExchangeLimits::MAXIMUM_RESPONSE_BYTES + 1),
    ] {
        let error = invalid.expect_err("limit must be rejected");
        assert_eq!(error.reason(), HttpsTransportErrorReason::InvalidLimit);
        assert_eq!(
            error.dispatch_outcome(),
            HttpsDispatchOutcome::NotDispatched
        );
    }

    let limits = HttpsExchangeLimits::new(2, 1).expect("valid limits");
    let error = BoundedHttpsRequest::new(
        HttpsMethod::Put,
        "wrp/value",
        b"too long",
        Duration::from_secs(1),
        limits,
    )
    .expect_err("body must be rejected");
    assert_eq!(
        error.reason(),
        HttpsTransportErrorReason::RequestLimitExceeded
    );
}

#[test]
fn rejects_invalid_timeouts_and_headers_before_dispatch() {
    let limits = HttpsExchangeLimits::new(1, 1).expect("valid limits");
    let error = BoundedHttpsRequest::new(
        HttpsMethod::Delete,
        "wrp/value",
        b"",
        Duration::ZERO,
        limits,
    )
    .expect_err("timeout must be rejected");
    assert_eq!(error.reason(), HttpsTransportErrorReason::InvalidTimeout);
    assert_eq!(
        error.dispatch_outcome(),
        HttpsDispatchOutcome::NotDispatched
    );

    assert!(CapturedResponseHeader::new("invalid header", 1).is_err());
    assert!(CapturedResponseHeader::new("x-value", 0).is_err());

    let request = BoundedHttpsRequest::new(
        HttpsMethod::Post,
        "/value",
        b"x",
        Duration::from_secs(1),
        limits,
    )
    .expect("valid request");
    assert!(request.with_accept("invalid\nvalue").is_err());
}

#[test]
fn rejects_invalid_targets_and_redacts_valid_request_content() {
    let limits = HttpsExchangeLimits::new(64, 64).expect("valid limits");
    let error = BoundedHttpsRequest::new(
        HttpsMethod::Post,
        "//untrusted.example/path",
        b"secret",
        Duration::from_secs(1),
        limits,
    )
    .expect_err("authority-changing target must be rejected");
    assert_eq!(error.reason(), HttpsTransportErrorReason::InvalidTarget);
    assert_eq!(
        error.dispatch_outcome(),
        HttpsDispatchOutcome::NotDispatched
    );

    let request = BoundedHttpsRequest::new(
        HttpsMethod::Post,
        "/private/identifier",
        b"secret-body",
        Duration::from_secs(1),
        limits,
    )
    .expect("valid request");
    let output = format!("{request:?}");
    assert!(!output.contains("identifier"));
    assert!(!output.contains("secret-body"));
    assert!(output.contains("redacted"));
}

#[test]
fn accepts_reviewed_expanded_targets_and_rejects_the_hard_limit_plus_one() {
    const REVIEWED_EXPANDED_TARGET_BYTES: usize = 6_148;
    let limits = HttpsExchangeLimits::new(1, 1).expect("valid limits");
    let mut reviewed_target = String::from("wrp/");
    let suffix_bytes = REVIEWED_EXPANDED_TARGET_BYTES
        .checked_sub(reviewed_target.len())
        .expect("prefix fits reviewed target bound");
    reviewed_target.push_str(&"a".repeat(suffix_bytes));
    assert_eq!(reviewed_target.len(), REVIEWED_EXPANDED_TARGET_BYTES);
    BoundedHttpsRequest::new(
        HttpsMethod::Delete,
        &reviewed_target,
        b"",
        Duration::from_secs(1),
        limits,
    )
    .expect("reviewed expanded target must fit the platform bound");

    let oversized_bytes = BoundedHttpsRequest::MAXIMUM_TARGET_BYTES
        .checked_add(1)
        .expect("hard target bound fits usize");
    let oversized_target = "a".repeat(oversized_bytes);
    let error = BoundedHttpsRequest::new(
        HttpsMethod::Delete,
        &oversized_target,
        b"",
        Duration::from_secs(1),
        limits,
    )
    .expect_err("target above the hard limit must be rejected");
    assert_eq!(error.reason(), HttpsTransportErrorReason::InvalidTarget);
    assert_eq!(
        error.dispatch_outcome(),
        HttpsDispatchOutcome::NotDispatched
    );
}
