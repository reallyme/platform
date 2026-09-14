// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::Infallible;
use std::future;
use std::time::Duration;

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;

use super::HttpTimeoutResponseFuture;

#[tokio::test]
async fn timeout_future_prefers_inner_completion_when_timeout_is_also_ready() {
    let response = Response::builder()
        .status(StatusCode::OK)
        .body(Body::empty())
        .expect("test response should build");
    let inner = future::ready(Ok::<Response, Infallible>(response));

    let result = HttpTimeoutResponseFuture::new(inner, Duration::ZERO, None).await;

    let response = result.expect("timeout future should not fail");
    assert_eq!(response.status(), StatusCode::OK);
}
