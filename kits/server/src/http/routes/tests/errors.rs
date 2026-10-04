// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::http::{Method, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, options, post};
use serde_json::json;

use super::super::apply_standard_router_layers;
use super::fixtures::{
    BlockedRouteState, assert_stable_json_error_response, blocked_route,
    explicit_json_not_found_route, json_body_route, mapped_internal_error_route, options_route,
    oversized_body_route, slow_route, test_http_config, test_http_config_with_concurrency,
    test_http_config_with_security, test_http_security_allowing_host,
};
use crate::http::TestServer;

#[cfg(feature = "websocket")]
#[tokio::test]
async fn unsupported_websocket_version_advertises_version_thirteen() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route(
            "/ws",
            get(|_upgrade: axum::extract::ws::WebSocketUpgrade| async {}),
        ),
        &config,
    );
    let server = TestServer::new(app);
    let response = server
        .get("/ws")
        .add_header(header::CONNECTION, "Upgrade")
        .add_header(header::UPGRADE, "websocket")
        .add_header(header::SEC_WEBSOCKET_VERSION, "12")
        .add_header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
    response.assert_header(header::SEC_WEBSOCKET_VERSION, "13");
}

#[tokio::test]
async fn non_json_not_found_is_normalized_to_stable_error_envelope() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(Router::new(), &config);
    let server = TestServer::new(app);

    let response = server.get("/missing").await;

    assert_stable_json_error_response(&response, StatusCode::NOT_FOUND, "not_found", "Not found");
}

#[tokio::test]
async fn non_json_method_not_allowed_is_normalized_to_stable_error_envelope() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app =
        apply_standard_router_layers(Router::new().route("/get-only", get(|| async {})), &config);
    let server = TestServer::new(app);

    let response = server.post("/get-only").await;

    response.assert_contains_header(header::ALLOW);
    assert_stable_json_error_response(
        &response,
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "Method not allowed",
    );
}

#[tokio::test]
async fn explicit_json_error_responses_are_not_rewrapped() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route(
            "/explicit-json-not-found",
            get(explicit_json_not_found_route),
        ),
        &config,
    );
    let server = TestServer::new(app);

    let response = server.get("/explicit-json-not-found").await;

    response.assert_status(StatusCode::NOT_FOUND);
    response.assert_json(&json!({
        "error": {
            "code": "explicit_not_found",
            "message": "Explicit not found"
        }
    }));
}

#[tokio::test]
async fn timeout_layer_returns_stable_public_error_without_internal_leakage() {
    let config = test_http_config(Duration::from_millis(10), 1024);
    let app = apply_standard_router_layers(Router::new().route("/slow", get(slow_route)), &config);
    let server = TestServer::new(app);

    let response = server.get("/slow").await;

    assert_stable_json_error_response(
        &response,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "Service unavailable",
    );
}

#[tokio::test]
async fn body_limit_rejects_oversized_requests() {
    let config = test_http_config(Duration::from_secs(1), 4);
    let app = apply_standard_router_layers(
        Router::new().route("/echo-body", post(oversized_body_route)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server.post("/echo-body").text("abcdef").await;

    assert_stable_json_error_response(
        &response,
        StatusCode::PAYLOAD_TOO_LARGE,
        "payload_too_large",
        "Payload too large",
    );
}

#[tokio::test]
async fn malformed_json_maps_to_stable_bad_request_error() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route("/json-body", post(json_body_route)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server
        .post("/json-body")
        .bytes(Bytes::from_static(b"{ malformed json"))
        .content_type("application/json")
        .await;

    assert_stable_json_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "bad_request",
        "Bad request",
    );
    let body_text = response.text();

    assert!(!body_text.contains("expected ident"));
    assert!(!body_text.contains("JsonRejection"));
}

#[tokio::test]
async fn invalid_json_value_maps_to_stable_unprocessable_entity() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route("/json-body", post(json_body_route)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server
        .post("/json-body")
        .bytes(Bytes::from_static(br#"{"value":123}"#))
        .content_type("application/json")
        .await;

    assert_stable_json_error_response(
        &response,
        StatusCode::UNPROCESSABLE_ENTITY,
        "unprocessable_entity",
        "Unprocessable entity",
    );
    assert!(!response.text().contains("invalid type"));
}

#[tokio::test]
async fn normalized_auth_error_preserves_challenge_without_raw_body() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route(
            "/auth",
            get(|| async {
                (
                    StatusCode::UNAUTHORIZED,
                    [(header::WWW_AUTHENTICATE, "Bearer realm=\"example\"")],
                    "internal auth detail",
                )
            }),
        ),
        &config,
    );
    let server = TestServer::new(app);

    let response = server.get("/auth").await;

    response.assert_contains_header(header::WWW_AUTHENTICATE);
    assert_stable_json_error_response(
        &response,
        StatusCode::UNAUTHORIZED,
        "unauthorized",
        "Unauthorized",
    );
    assert!(!response.text().contains("internal auth detail"));
}

#[tokio::test]
async fn normalized_auth_error_preserves_every_authenticate_challenge() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route(
            "/auth-many",
            get(|| async {
                let mut response = (StatusCode::UNAUTHORIZED, "private detail").into_response();
                response.headers_mut().append(
                    header::WWW_AUTHENTICATE,
                    "Bearer realm=\"api\"".parse().expect("valid challenge"),
                );
                response.headers_mut().append(
                    header::WWW_AUTHENTICATE,
                    "Basic realm=\"legacy\"".parse().expect("valid challenge"),
                );
                response
            }),
        ),
        &config,
    );
    let response = TestServer::new(app).get("/auth-many").await;
    let challenges: Vec<_> = response
        .headers()
        .get_all(header::WWW_AUTHENTICATE)
        .iter()
        .collect();
    assert_eq!(challenges.len(), 2);
    assert_eq!(challenges[0], "Bearer realm=\"api\"");
    assert_eq!(challenges[1], "Basic realm=\"legacy\"");
    assert!(!response.text().contains("private detail"));
}

#[tokio::test]
async fn unsupported_json_media_type_maps_to_stable_error() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route("/json-body", post(json_body_route)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server.post("/json-body").text("not json").await;

    assert_stable_json_error_response(
        &response,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
        "Unsupported media type",
    );
    let body_text = response.text();

    assert!(!body_text.contains("Expected request with"));
    assert!(!body_text.contains("JsonRejection"));
}

#[tokio::test]
async fn connect_unsupported_media_type_preserves_accept_post() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route(
            "/service/Call",
            post(|| async {
                (
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    [("accept-post", "application/proto, application/json")],
                )
            }),
        ),
        &config,
    );
    let response = TestServer::new(app).post("/service/Call").await;

    response.assert_status(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    response.assert_contains_header("accept-post");
    assert!(response.text().is_empty());
}

#[tokio::test]
async fn concurrency_limit_returns_stable_service_unavailable_error() {
    let config = test_http_config_with_concurrency(Duration::from_secs(5), 1024, 1);
    let state = Arc::new(BlockedRouteState::new());
    let app = apply_standard_router_layers(
        Router::new()
            .route("/blocked", get(blocked_route))
            .with_state(Arc::clone(&state)),
        &config,
    );
    let server = Arc::new(TestServer::new(app));
    tokio::task::LocalSet::new()
        .run_until(async move {
            let first_server = Arc::clone(&server);
            let first_request =
                tokio::task::spawn_local(async move { first_server.get("/blocked").await });

            tokio::time::timeout(Duration::from_secs(2), state.entered.notified())
                .await
                .expect("first request should enter the handler before overload probe");
            assert_eq!(state.entered_count(), 1);

            let overloaded = server.get("/blocked").await;
            assert_stable_json_error_response(
                &overloaded,
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "Service unavailable",
            );

            state.release.notify_one();
            let first_response = tokio::time::timeout(Duration::from_secs(2), first_request)
                .await
                .expect("first request should complete after release")
                .expect("first request task should join");
            first_response.assert_status_ok();
        })
        .await;
}

#[tokio::test]
async fn internal_error_mapping_does_not_leak_internal_details() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route("/internal-error", get(mapped_internal_error_route)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server.get("/internal-error").await;

    assert_stable_json_error_response(
        &response,
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_server_error",
        "Internal server error",
    );
    let body = response.text();
    assert!(body.contains("\"internal_server_error\""));
    assert!(body.contains("\"Internal server error\""));
    assert!(!body.contains("InternalDependencyFailure"));
    assert!(!body.contains("dependency"));
}

#[tokio::test]
async fn default_cors_posture_is_not_permissive() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route("/cors-probe", options(options_route)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server
        .method(Method::OPTIONS, "/cors-probe")
        .add_header(header::ORIGIN, "https://example.com")
        .add_header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .await;

    response.assert_status(StatusCode::NO_CONTENT);
    assert!(!response.contains_header(header::ACCESS_CONTROL_ALLOW_ORIGIN));
}

#[tokio::test]
async fn standard_layers_add_security_headers() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(Router::new().route("/ok", get(|| async {})), &config);
    let server = TestServer::new(app);

    let response = server.get("/ok").await;

    response.assert_status_ok();
    response.assert_header("x-content-type-options", "nosniff");
    response.assert_header("referrer-policy", "strict-origin-when-cross-origin");
    response.assert_header("x-frame-options", "DENY");
    response.assert_header("content-security-policy", "frame-ancestors 'none'");
}

#[tokio::test]
async fn host_authority_allowlist_blocks_unexpected_hosts() {
    let security = test_http_security_allowing_host("api.reallyme.net");
    let config = test_http_config_with_security(Duration::from_secs(1), 1024, security);
    let app = apply_standard_router_layers(Router::new().route("/ok", get(|| async {})), &config);
    let server = TestServer::new(app);

    let response = server
        .get("/ok")
        .add_header(header::HOST, "evil.example")
        .await;

    assert_stable_json_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "bad_request",
        "Bad request",
    );
    response.assert_header("cache-control", "no-store");
}

#[tokio::test]
async fn cors_preflight_does_not_bypass_host_security() {
    let security = test_http_security_allowing_host("api.reallyme.net");
    let config = test_http_config_with_security(Duration::from_secs(1), 1024, security);
    let app = apply_standard_router_layers(
        Router::new().route("/cors-probe", options(options_route)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server
        .method(Method::OPTIONS, "/cors-probe")
        .add_header(header::HOST, "evil.example")
        .add_header(header::ORIGIN, "https://app.reallyme.net")
        .add_header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .await;

    assert_stable_json_error_response(
        &response,
        StatusCode::BAD_REQUEST,
        "bad_request",
        "Bad request",
    );
    response.assert_header("x-content-type-options", "nosniff");
}
