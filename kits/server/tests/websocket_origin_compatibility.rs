// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![cfg(feature = "websocket")]

use axum::http::{HeaderMap, HeaderValue, header};
use reallyme_server_kit::http::{ExternalRequestOrigin, websocket_same_origin};

#[test]
fn published_websocket_origin_check_keeps_the_previous_call_signature() {
    let check: fn(&HeaderMap, Option<&ExternalRequestOrigin>) -> bool = websocket_same_origin;
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("api.example.com"));
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("http://api.example.com"),
    );
    assert!(check(&headers, None));
}
