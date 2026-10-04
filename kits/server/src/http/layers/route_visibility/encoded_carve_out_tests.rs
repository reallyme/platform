// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
