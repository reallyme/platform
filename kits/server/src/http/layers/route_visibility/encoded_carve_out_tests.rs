// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use tower::{Layer, ServiceExt};

use crate::http::{
    HttpListenerName, HttpListenerVisibility, HttpRoutePrefix, HttpRouteVisibility,
    HttpRouteVisibilityPolicy, HttpRouteVisibilityRule,
};

use super::route_visibility_layer;

#[tokio::test]
async fn encoded_exact_carve_out_cannot_reach_private_parameter_handler() {
    let handler_executed = Arc::new(AtomicBool::new(false));
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![
        HttpRouteVisibilityRule::new(
            HttpRoutePrefix::new("/admin").expect("valid prefix"),
            HttpRouteVisibility::InternalOnly,
        ),
        HttpRouteVisibilityRule::exact(
            HttpRoutePrefix::new("/admin/health").expect("valid prefix"),
            HttpRouteVisibility::PublicOnly,
        ),
    ])
    .expect("unique route rules");
    let app = Router::new()
        .route("/admin/health", get(|| async { StatusCode::OK }))
        .route(
            "/admin/{id}",
            get(|State(executed): State<Arc<AtomicBool>>| async move {
                executed.store(true, Ordering::SeqCst);
                StatusCode::OK
            }),
        )
        .with_state(handler_executed.clone());
    let service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .oneshot(
            Request::builder()
                .uri("/admin/%68ealth")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("infallible service");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(!handler_executed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn both_path_spellings_must_agree_even_when_each_rule_permits_the_listener() {
    let public_listener = HttpListenerName::new("public").expect("valid listener");
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![
        HttpRouteVisibilityRule::new(
            HttpRoutePrefix::new("/files").expect("valid prefix"),
            HttpRouteVisibility::PublicOnly,
        )
        .with_allowed_listeners(vec![public_listener.clone()]),
        HttpRouteVisibilityRule::exact(
            HttpRoutePrefix::new("/files/status").expect("valid exact route"),
            HttpRouteVisibility::PublicOnly,
        )
        .with_allowed_listeners(vec![public_listener.clone()]),
    ])
    .expect("unique route rules");
    let app = Router::new().fallback(get(|| async { StatusCode::NO_CONTENT }));

    for (listener, path, expected) in [
        ("public", "/files/status", StatusCode::NO_CONTENT),
        ("public", "/files/%73tatus", StatusCode::FORBIDDEN),
        ("public", "/files/%61rchive", StatusCode::NO_CONTENT),
        ("other", "/files/%61rchive", StatusCode::FORBIDDEN),
        ("other", "/%61rchive", StatusCode::NO_CONTENT),
    ] {
        let response = route_visibility_layer(
            HttpListenerName::new(listener).expect("valid listener"),
            HttpListenerVisibility::Public,
            None,
            Arc::new(Vec::new()),
            &policy,
        )
        .layer(app.clone())
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("infallible service");
        assert_eq!(response.status(), expected, "{listener} {path}");
    }
}
