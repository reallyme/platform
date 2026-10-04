// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{PreflightCorsLayer, app_cors_layer, base_cors_layer};
use crate::config::CorsConfig;
use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::routing::options;
use reallyme_app_kit::AppJsoncConfigDocument;
use tower::ServiceExt;
use tower_http::cors::AllowOrigin;

#[test]
fn no_cors_is_the_default_safe_posture() {
    assert!(CorsConfig::no_cors().is_disabled());
}

#[test]
fn app_cors_layer_is_none_when_no_origins_declared() {
    let document = AppJsoncConfigDocument::<reallyme_app_kit::NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": []},
            "reflection_enabled": false,
            "downstream": {}
        }"#,
    )
    .expect("valid empty-cors fixture");
    assert!(app_cors_layer(document.cors()).is_none());
}

#[test]
fn app_cors_layer_is_some_when_origins_declared() {
    let document = AppJsoncConfigDocument::<reallyme_app_kit::NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": ["https://app.reallyme.net"]},
            "reflection_enabled": false,
            "downstream": {}
        }"#,
    )
    .expect("valid single-origin fixture");
    assert!(app_cors_layer(document.cors()).is_some());
}

#[tokio::test]
async fn ordinary_options_reaches_handler_while_browser_preflight_is_answered() {
    let layer = PreflightCorsLayer::new(base_cors_layer().allow_origin(AllowOrigin::exact(
        "https://app.example.com".parse().expect("valid origin"),
    )));
    let router = Router::new()
        .route("/probe", options(|| async { StatusCode::NO_CONTENT }))
        .layer(layer);

    let ordinary = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/probe")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("infallible router");
    assert_eq!(ordinary.status(), StatusCode::NO_CONTENT);

    let preflight = router
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/probe")
                .header(header::ORIGIN, "https://app.example.com")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("infallible router");
    assert_eq!(preflight.status(), StatusCode::OK);
    assert_eq!(
        preflight.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&"https://app.example.com".parse().expect("valid origin")),
    );
}
