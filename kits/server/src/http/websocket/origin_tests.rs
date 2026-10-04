// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::{HeaderMap, HeaderValue, header};

use super::websocket_same_origin;

#[test]
fn websocket_origin_check_rejects_cross_site_and_malformed_origins() {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("api.example.com"));
    assert!(websocket_same_origin(&headers, None));

    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("http://api.example.com"),
    );
    assert!(websocket_same_origin(&headers, None));

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
        assert!(!websocket_same_origin(&headers, None), "{origin}");
    }
}
