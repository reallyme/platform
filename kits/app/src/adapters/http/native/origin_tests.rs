// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::HttpsOrigin;
use crate::{HttpsDispatchOutcome, HttpsTransportErrorReason};

#[test]
fn accepts_authority_only_https_origins_and_exact_relative_targets() {
    let origin = HttpsOrigin::try_from("https://service.example:8443/").expect("valid origin");
    let absolute_path = origin.resolve("/wallet_rp/a%2Fb").expect("valid target");
    assert_eq!(
        absolute_path.as_str(),
        "https://service.example:8443/wallet_rp/a%2Fb"
    );

    let relative_path = origin.resolve("wrp/value").expect("valid target");
    assert_eq!(
        relative_path.as_str(),
        "https://service.example:8443/wrp/value"
    );
}

#[test]
fn rejects_origins_with_plaintext_credentials_or_url_state() {
    for invalid in [
        "http://service.example/",
        "https://user@service.example/",
        "https://service.example/path",
        "https://service.example/?query=true",
        "https://service.example/#fragment",
        "not a URL",
    ] {
        let error = HttpsOrigin::try_from(invalid).expect_err("origin must be rejected");
        assert_eq!(error.reason(), HttpsTransportErrorReason::InvalidOrigin);
        assert_eq!(
            error.dispatch_outcome(),
            HttpsDispatchOutcome::NotDispatched
        );
    }
}

#[test]
fn rejects_targets_that_can_escape_or_be_normalized() {
    let origin = HttpsOrigin::try_from("https://service.example/").expect("valid origin");
    for invalid in [
        "",
        "//other.example/path",
        "https://other.example/path",
        "/path?query=true",
        "/path#fragment",
        "/path\\child",
        "/wallet/../secret",
        "/wallet/%2e%2e/secret",
        "/wallet/.%2E/secret",
        "/wallet/%",
        "/wallet/%2",
        "/wallet/%zz",
        "/wallet/contains space",
        "/wallet/é",
    ] {
        let error = origin
            .resolve(invalid)
            .expect_err("target must be rejected");
        assert_eq!(error.reason(), HttpsTransportErrorReason::InvalidTarget);
        assert_eq!(
            error.dispatch_outcome(),
            HttpsDispatchOutcome::NotDispatched
        );
    }
}

#[test]
fn debug_output_does_not_expose_the_origin() {
    let origin = HttpsOrigin::try_from("https://secret-service.example/").expect("valid origin");
    let output = format!("{origin:?}");
    assert!(!output.contains("secret-service"));
    assert!(output.contains("redacted"));
}
