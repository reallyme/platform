// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use serde_json::json;

use super::super::apply_standard_router_layers;
use super::fixtures::{
    custom_correlation_headers_route, echo_request_id, echo_trace_id, test_http_config,
};
use crate::http::{
    HttpListenerName, HttpListenerVisibility, RequestId, TestServer, TraceId, X_REQUEST_ID,
    X_TRACE_ID, listener_identity_layer,
};

#[tokio::test]
async fn public_request_id_is_regenerated_even_when_input_is_valid() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let request_id = RequestId::generate();
    let request_id_header = request_id
        .to_header_value()
        .expect("generated request ID should format as header");
    let app = apply_standard_router_layers(
        Router::new().route("/echo-request-id", get(echo_request_id)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server
        .get("/echo-request-id")
        .add_header(
            X_REQUEST_ID,
            request_id_header
                .to_str()
                .expect("header should be visible ascii"),
        )
        .await;

    response.assert_status_ok();
    let response_id = response
        .headers()
        .get(X_REQUEST_ID)
        .expect("response request ID should be present");
    assert_ne!(response_id, request_id_header);
    response.assert_json(&json!({ "request_id": response_id.to_str().expect("ASCII UUID") }));
}

#[tokio::test]
async fn public_trace_id_is_regenerated_even_when_input_is_valid() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let trace_id = TraceId::generate();
    let trace_id_header = trace_id
        .to_header_value()
        .expect("generated trace ID should format as header");
    let app = apply_standard_router_layers(
        Router::new().route("/echo-trace-id", get(echo_trace_id)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server
        .get("/echo-trace-id")
        .add_header(
            X_TRACE_ID,
            trace_id_header
                .to_str()
                .expect("header should be visible ascii"),
        )
        .await;

    response.assert_status_ok();
    let response_id = response
        .headers()
        .get(X_TRACE_ID)
        .expect("response trace ID should be present");
    assert_ne!(response_id, trace_id_header);
    response.assert_json(&json!({ "trace_id": response_id.to_str().expect("ASCII UUID") }));
}

#[tokio::test]
async fn internal_listener_preserves_valid_inbound_request_id() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let request_id = RequestId::generate();
    let app = apply_standard_router_layers(
        Router::new().route("/echo-request-id", get(echo_request_id)),
        &config,
    )
    .layer(listener_identity_layer(
        HttpListenerName::new("internal").expect("valid listener name"),
        HttpListenerVisibility::Internal,
        SocketAddr::from(([127, 0, 0, 1], 8080)),
    ));
    let response = TestServer::new(app)
        .get("/echo-request-id")
        .add_header(X_REQUEST_ID, request_id.into_uuid().to_string())
        .await;

    response.assert_status_ok();
    response.assert_header(X_REQUEST_ID, request_id.into_uuid().to_string());
}

#[tokio::test]
async fn malformed_request_id_is_replaced() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route("/echo-request-id", get(echo_request_id)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server
        .get("/echo-request-id")
        .add_header(X_REQUEST_ID, "not-a-valid-uuid")
        .await;

    response.assert_status_ok();
    response.assert_contains_header(X_REQUEST_ID);

    let body = response.json::<serde_json::Value>();
    let response_request_id = body["request_id"]
        .as_str()
        .expect("response request_id should be present");

    assert_ne!(response_request_id, "missing");
    assert_ne!(response_request_id, "not-a-valid-uuid");
    assert!(uuid::Uuid::parse_str(response_request_id).is_ok());
}

#[tokio::test]
async fn malformed_trace_id_is_replaced() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route("/echo-trace-id", get(echo_trace_id)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server
        .get("/echo-trace-id")
        .add_header(X_TRACE_ID, "not-a-valid-uuid")
        .await;

    response.assert_status_ok();
    response.assert_contains_header(X_TRACE_ID);

    let body = response.json::<serde_json::Value>();
    let response_trace_id = body["trace_id"]
        .as_str()
        .expect("response trace_id should be present");

    assert_ne!(response_trace_id, "missing");
    assert_ne!(response_trace_id, "not-a-valid-uuid");
    assert!(uuid::Uuid::parse_str(response_trace_id).is_ok());
}

#[tokio::test]
async fn response_correlation_headers_set_by_app_are_not_overwritten() {
    let config = test_http_config(Duration::from_secs(1), 1024);
    let app = apply_standard_router_layers(
        Router::new().route("/custom-correlation", get(custom_correlation_headers_route)),
        &config,
    );
    let server = TestServer::new(app);

    let response = server.get("/custom-correlation").await;

    response.assert_status(StatusCode::NO_CONTENT);
    let request_id = response
        .headers()
        .get(X_REQUEST_ID)
        .expect("response request ID should be present");
    let trace_id = response
        .headers()
        .get(X_TRACE_ID)
        .expect("response trace ID should be present");

    assert_ne!(request_id, "missing");
    assert_ne!(trace_id, "missing");
    assert!(RequestId::try_from(request_id).is_ok());
    assert!(TraceId::try_from(trace_id).is_ok());
}
