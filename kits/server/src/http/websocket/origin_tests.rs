// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::{HeaderMap, HeaderValue, Uri, header};

use super::websocket_same_origin;

#[test]
fn websocket_origin_check_rejects_cross_site_and_malformed_origins() {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("api.example.com"));
    assert!(websocket_same_origin(
        &headers,
        &Uri::from_static("/ws"),
        None
    ));

    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("http://api.example.com"),
    );
    assert!(websocket_same_origin(
        &headers,
        &Uri::from_static("/ws"),
        None
    ));

    for origin in [
        "https://api.example.com",
        "http://evil.example.com",
        "http://api.example.com.evil.test",
        "http://api.example.com/other",
        "null",
    ] {
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_str(origin).expect("test origin header"),
        );
        assert!(
            !websocket_same_origin(&headers, &Uri::from_static("/ws"), None),
            "{origin}"
        );
    }
}

#[test]
fn websocket_origin_uses_http2_authority_and_scheme_without_host() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://api.example.com"),
    );
    let uri = Uri::from_static("https://api.example.com/ws");
    assert!(websocket_same_origin(&headers, &uri, None));

    headers.insert(header::HOST, HeaderValue::from_static("other.example.com"));
    assert!(!websocket_same_origin(&headers, &uri, None));
}
