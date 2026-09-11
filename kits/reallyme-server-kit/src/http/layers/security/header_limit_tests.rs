// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::security_layer;
use crate::config::{
    HttpHeaderBytesLimit, HttpHeaderCountLimit, HttpHeaderLimitConfig, HttpSecurityConfig,
};
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use std::convert::Infallible;
use tower::{Layer, ServiceExt, service_fn};

#[tokio::test]
async fn proxy_header_bytes_are_counted_before_the_headers_are_stripped() {
    let limits = HttpHeaderLimitConfig::new(
        HttpHeaderCountLimit::new(16).expect("count"),
        HttpHeaderBytesLimit::new(1024).expect("bytes"),
    );
    let config = HttpSecurityConfig::default().with_header_limits(limits);
    let service = security_layer(&config).layer(service_fn(|_: Request<Body>| async {
        Ok::<_, Infallible>(Response::new(Body::empty()))
    }));
    let request = Request::builder()
        .uri("/hello")
        .header("x-forwarded-for", "x".repeat(1025))
        .body(Body::empty())
        .expect("request");
    let response = service.oneshot(request).await.expect("infallible");
    assert_eq!(
        response.status(),
        StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE
    );
}
