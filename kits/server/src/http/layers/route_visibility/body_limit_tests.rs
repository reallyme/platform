// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use tower::{Layer, Service, ServiceExt};

use crate::http::{
    HttpListenerName, HttpListenerVisibility, HttpRoutePrefix, HttpRouteVisibility,
    HttpRouteVisibilityPolicy, HttpRouteVisibilityRule, RequestBodyLimitBytes,
};

use super::route_visibility_layer;

#[tokio::test]
async fn route_visibility_rejects_request_exceeding_route_body_limit() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![
        HttpRouteVisibilityRule::exact(
            HttpRoutePrefix::new("/handles/handle-availability/check").expect("valid route prefix"),
            HttpRouteVisibility::PublicOnly,
        )
        .with_request_body_limit(Some(
            RequestBodyLimitBytes::new(1024).expect("valid route body limit"),
        )),
    ])
    .expect("unique route rule");
    let app = Router::new().route(
        "/handles/handle-availability/check",
        get(|| async { StatusCode::OK }),
    );
    let mut service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener name"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/handles/handle-availability/check")
                .header("content-length", "2048")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn route_visibility_bounds_unadvertised_bodies_and_rejects_bad_lengths() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![
        HttpRouteVisibilityRule::exact(
            HttpRoutePrefix::new("/upload").expect("valid route prefix"),
            HttpRouteVisibility::PublicOnly,
        )
        .with_request_body_limit(Some(RequestBodyLimitBytes::new(4).expect("valid limit"))),
    ])
    .expect("valid route policy");
    let app = Router::new().route("/upload", post(|body: String| async move { body }));
    let service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let oversized = service
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/upload")
                .body(Body::from("12345"))
                .expect("valid request"),
        )
        .await
        .expect("infallible service");
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let malformed = service
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/upload")
                .header("content-length", "not-a-number")
                .body(Body::from("123"))
                .expect("valid request"),
        )
        .await
        .expect("infallible service");
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);

    let accepted = service
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/upload")
                .body(Body::from("1234"))
                .expect("valid request"),
        )
        .await
        .expect("infallible service");
    assert_eq!(accepted.status(), StatusCode::OK);
}
