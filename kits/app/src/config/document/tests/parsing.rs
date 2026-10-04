// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::super::{AppConfigDocumentErrorReason, AppConfigDocumentField};
use crate::{AppConfigParseErrorReason, AppJsoncConfigDocument, NoAppCustomConfig};

#[test]
fn standard_app_jsonc_document_parses_expected_shape() {
    let document = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "public_base_url": "https://api.reallyme.net",
            "cors": {
                "allowed_origins": ["https://app.reallyme.net"]
            },
            "cookies": {
                "secure": true,
                "domain": "reallyme.net",
                "same_site_policy": "lax"
            },
            "reflection_enabled": false,
            "downstream": {
                "handle_policy": {
                    "base_url": "http://127.0.0.1:7001"
                }
            }
        }"#,
    )
    .expect("valid standard app JSONC fixture should parse");

    assert_eq!(
        document
            .public_base_url()
            .map(super::super::AppBaseUrl::as_str),
        Some("https://api.reallyme.net"),
    );
    assert_eq!(document.downstream().endpoints().len(), 1);
    assert!(document.cookies().is_some());
}

#[test]
fn rejects_unknown_top_level_timeout_field_for_no_custom_config() {
    let result = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": ["https://app.reallyme.net"]},
            "timeouts": {"request_timeout_millis": 30000},
            "reflection_enabled": false,
            "downstream": {}
        }"#,
    );

    assert_eq!(
        result.map_err(|error| error.reason()),
        Err(AppConfigDocumentErrorReason::Parse {
            reason: AppConfigParseErrorReason::InvalidJson,
        }),
    );
}

#[test]
fn rejects_unknown_nested_cookie_and_cors_fields() {
    for document in [
        r#"{"cors":{"allowed_origins":[],"allow_any":true},"reflection_enabled":false,"downstream":{}}"#,
        r#"{"cors":{"allowed_origins":[]},"cookies":{"same_site":"strict"},"reflection_enabled":false,"downstream":{}}"#,
    ] {
        let result = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(document);
        assert_eq!(
            result.map_err(|error| error.reason()),
            Err(AppConfigDocumentErrorReason::Parse {
                reason: AppConfigParseErrorReason::InvalidJson,
            }),
        );
    }
}

#[test]
fn rejects_single_label_cookie_domains_except_localhost() {
    for domain in ["com", "internal", "123"] {
        let document = format!(
            r#"{{"cors":{{"allowed_origins":[]}},"cookies":{{"domain":"{domain}"}},"reflection_enabled":false,"downstream":{{}}}}"#
        );
        let result = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(&document);
        assert_eq!(
            result.map_err(|error| error.reason()),
            Err(AppConfigDocumentErrorReason::InvalidCookieDomain),
        );
    }
}

#[test]
fn rejects_unknown_top_level_limits_field_for_no_custom_config() {
    let result = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": ["https://app.reallyme.net"]},
            "limits": {
                "request_body_limit_bytes": 1048576,
                "max_inflight_requests": 10000
            },
            "reflection_enabled": false,
            "downstream": {}
        }"#,
    );

    assert_eq!(
        result.map_err(|error| error.reason()),
        Err(AppConfigDocumentErrorReason::Parse {
            reason: AppConfigParseErrorReason::InvalidJson,
        }),
    );
}

#[test]
fn rejects_reflection_enabled_until_host_policy_exists() {
    let result = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": ["https://app.reallyme.net"]},
            "reflection_enabled": true,
            "downstream": {}
        }"#,
    );

    assert_eq!(
        result.map_err(|error| error.reason()),
        Err(AppConfigDocumentErrorReason::ReflectionNotSupported),
    );
}

#[test]
fn accepts_empty_allowed_origins_as_disabled_cors() {
    let document = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": []},
            "reflection_enabled": false,
            "downstream": {}
        }"#,
    )
    .expect("empty allowed_origins should map to disabled CORS");

    assert_eq!(document.cors().allowed_origins().len(), 0);
    assert!(!document.cors().is_enabled());
}

#[test]
fn parses_cookie_transport_policy() {
    let document = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": ["https://app.reallyme.net"]},
            "cookies": {
                "secure": true,
                "domain": "www.reallyme.net",
                "same_site_policy": "strict"
            },
            "reflection_enabled": false,
            "downstream": {}
        }"#,
    )
    .expect("valid cookie config should parse");

    let cookies = document.cookies().expect("cookies should be present");
    assert!(cookies.secure());
    assert_eq!(
        cookies.domain().map(super::super::AppCookieDomain::as_str),
        Some("www.reallyme.net")
    );
    assert_eq!(
        cookies.same_site_policy(),
        super::super::AppCookieSameSitePolicy::Strict
    );
}

#[test]
fn rejects_same_site_none_without_secure() {
    let result = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": ["https://app.reallyme.net"]},
            "cookies": {
                "secure": false,
                "same_site_policy": "none"
            },
            "reflection_enabled": false,
            "downstream": {}
        }"#,
    );

    assert_eq!(
        result.map_err(|error| error.reason()),
        Err(AppConfigDocumentErrorReason::CookieSameSiteNoneRequiresSecure),
    );
}

#[test]
fn validation_errors_identify_safe_field_paths() {
    for (document, field) in [
        (
            r#"{"public_base_url":"http://public.example","cors":{"allowed_origins":[]},"reflection_enabled":false,"downstream":{}}"#,
            AppConfigDocumentField::PublicBaseUrl,
        ),
        (
            r#"{"cors":{"allowed_origins":[]},"cookies":{"domain":"com"},"reflection_enabled":false,"downstream":{}}"#,
            AppConfigDocumentField::CookiesDomain,
        ),
        (
            r#"{"cors":{"allowed_origins":[]},"reflection_enabled":true,"downstream":{}}"#,
            AppConfigDocumentField::ReflectionEnabled,
        ),
        (
            r#"{"cors":{"allowed_origins":[]},"reflection_enabled":false,"downstream":{"bad:name":{"base_url":"http://127.0.0.1:7001"}}}"#,
            AppConfigDocumentField::Downstream,
        ),
    ] {
        let error = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(document)
            .expect_err("invalid field must be rejected");
        assert_eq!(error.field(), field);
        assert!(error.to_string().contains(&format!("{field:?}")));
        assert!(!error.to_string().contains("public.example"));
    }
}

#[test]
fn rejects_unknown_top_level_fields_when_no_custom_config_is_used() {
    let result = AppJsoncConfigDocument::<NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": ["https://app.reallyme.net"]},
            "reflection_enabled": false,
            "downstream": {},
            "timetouts": {"request_timeout_millis": 30000}
        }"#,
    );

    assert_eq!(
        result.map_err(|error| error.reason()),
        Err(AppConfigDocumentErrorReason::Parse {
            reason: AppConfigParseErrorReason::InvalidJson,
        }),
    );
}
