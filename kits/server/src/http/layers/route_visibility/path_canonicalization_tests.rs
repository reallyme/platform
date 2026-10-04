// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use tower::{Layer, Service, ServiceExt};

use crate::http::{
    HttpListenerName, HttpListenerVisibility, HttpRoutePrefix, HttpRouteVisibility,
    HttpRouteVisibilityPolicy, HttpRouteVisibilityRule,
};

use super::route_visibility_layer;

#[tokio::test]
async fn ambiguous_paths_cannot_bypass_private_visibility_rules() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![
        HttpRouteVisibilityRule::new(
            HttpRoutePrefix::new("/files/private").expect("valid prefix"),
            HttpRouteVisibility::PrivateOnly,
        ),
        HttpRouteVisibilityRule::new(
            HttpRoutePrefix::new("/admin").expect("valid prefix"),
            HttpRouteVisibility::PrivateOnly,
        ),
    ])
    .expect("unique rules");
    let mut service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(Router::new().fallback(get(|| async { StatusCode::OK })));

    for path in ["/files/%70rivate/key.pem", "/%61dmin/users"] {
        let response = service
            .ready()
            .await
            .expect("service ready")
            .call(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("valid URI fixture"),
            )
            .await
            .expect("infallible visibility service");
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "path: {path}");
    }

    for path in [
        "/files/private%2Fkey.pem",
        "/files//private/key.pem",
        "/files/./private/key.pem",
        "/files/public/../private/key.pem",
        "/files/%2e%2e/private/key.pem",
        "/files/%25%32%46private/key.pem",
    ] {
        let response = service
            .ready()
            .await
            .expect("service ready")
            .call(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("valid URI fixture"),
            )
            .await
            .expect("infallible visibility service");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "path: {path}");
    }
}

#[tokio::test]
async fn benign_escapes_are_allowed_without_visibility_rules() {
    let policy = HttpRouteVisibilityPolicy::allow_all();
    let mut service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(Router::new().fallback(get(|| async { StatusCode::OK })));

    for path in ["/users/alice%40example", "/hello%20world", "/caf%C3%A9"] {
        let response = service
            .ready()
            .await
            .expect("service ready")
            .call(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("valid URI fixture"),
            )
            .await
            .expect("infallible visibility service");
        assert_eq!(response.status(), StatusCode::OK, "path: {path}");
    }
}
