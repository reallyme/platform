// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use serde_json::json;

use crate::config::{
    BindAddress, BodyLimitConfig, ConcurrencyLimitConfigField, CorsConfig, HostAuthority,
    HostAuthorityPolicy, HttpSecurityConfig, HttpServerConfig, LogFormat, MetricsIdleTimeout,
    ObservabilityConfig, OperationalRouteAccess, RequestBodyLimitBytes, RequestTimeout,
    RuntimeConcurrencyLimit, SecurityHeadersConfig, ServiceEnvironment, TimeoutConfig,
    TrustedProxyHeaders, TrustedProxyRequestMetadataConfig,
};
use crate::http::{
    ErrorCode, PublicHttpError, RequestId, TestResponse, ToHttpErrorResponse, TraceId,
    X_REQUEST_ID, X_TRACE_ID, request_id_from_headers, trace_id_from_headers,
};

pub(super) const SUBPROCESS_HTTP_ROUTES_TEST_ENV: &str =
    "REALLYME_SERVER_KIT_HTTP_ROUTES_SUBPROCESS";

pub(super) fn test_http_config(timeout: Duration, body_limit_bytes: usize) -> HttpServerConfig {
    let bind_address =
        BindAddress::new(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080)))
            .expect("valid bind address");
    let timeout = TimeoutConfig::new(RequestTimeout::new(timeout).expect("valid request timeout"));
    let body_limit = BodyLimitConfig::new(
        RequestBodyLimitBytes::new(body_limit_bytes).expect("valid body limit"),
    );

    HttpServerConfig::new(bind_address, CorsConfig::no_cors(), timeout, body_limit)
}

pub(super) fn test_http_config_with_concurrency(
    timeout: Duration,
    body_limit_bytes: usize,
    in_flight_limit: usize,
) -> HttpServerConfig {
    let bind_address =
        BindAddress::new(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080)))
            .expect("valid bind address");
    let timeout = TimeoutConfig::new(RequestTimeout::new(timeout).expect("valid request timeout"));
    let body_limit = BodyLimitConfig::new(
        RequestBodyLimitBytes::new(body_limit_bytes).expect("valid body limit"),
    );
    let concurrency_limit = RuntimeConcurrencyLimit::new(
        in_flight_limit,
        ConcurrencyLimitConfigField::HttpInFlightRequests,
    )
    .expect("valid concurrency limit");

    HttpServerConfig::with_concurrency_limit(
        bind_address,
        CorsConfig::no_cors(),
        timeout,
        body_limit,
        concurrency_limit,
    )
}

pub(super) fn test_http_config_with_security(
    timeout: Duration,
    body_limit_bytes: usize,
    security: HttpSecurityConfig,
) -> HttpServerConfig {
    test_http_config(timeout, body_limit_bytes).with_security_config(security)
}

pub(super) fn test_http_security_allowing_host(host: &str) -> HttpSecurityConfig {
    HttpSecurityConfig::new(
        SecurityHeadersConfig::secure_defaults(),
        HostAuthorityPolicy::allow_list(vec![
            HostAuthority::new(host).expect("valid test host authority"),
        ])
        .expect("non-empty host allowlist"),
        TrustedProxyHeaders::ignore_all(),
        TrustedProxyRequestMetadataConfig::secure_defaults(),
        OperationalRouteAccess::Public,
    )
}

pub(super) fn test_observability_config() -> ObservabilityConfig {
    ObservabilityConfig::new(
        ServiceEnvironment::Local,
        LogFormat::PlainText,
        false,
        "reallyme_server_kit=info".to_owned(),
        MetricsIdleTimeout::new(Duration::from_secs(30)).expect("valid metrics timeout"),
    )
    .expect("valid observability config")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestMappedError {
    InternalDependencyFailure,
}

impl ToHttpErrorResponse for TestMappedError {
    fn public_http_error(&self) -> PublicHttpError {
        match self {
            Self::InternalDependencyFailure => {
                PublicHttpError::from_code(ErrorCode::InternalServerError)
            }
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct TestJsonPayload {
    value: String,
}

pub(super) fn assert_stable_json_error_response(
    response: &TestResponse,
    status: StatusCode,
    code: &'static str,
    message: &'static str,
) {
    response.assert_status(status);
    response.assert_header(header::CONTENT_TYPE, "application/json");

    let body = response.json::<serde_json::Value>();

    assert_eq!(body["error"]["code"], json!(code));
    assert_eq!(body["error"]["message"], json!(message));
    let request_id = body["error"]["request_id"]
        .as_str()
        .expect("error response should include request_id");
    assert!(
        uuid::Uuid::parse_str(request_id).is_ok(),
        "request_id should be a valid UUID: {request_id}"
    );
}

pub(super) async fn echo_request_id(
    headers: axum::http::HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    let request_id = request_id_from_headers(&headers)
        .map(|request_id| request_id.into_uuid().to_string())
        .unwrap_or_else(|| "missing".to_owned());

    (StatusCode::OK, Json(json!({ "request_id": request_id })))
}

pub(super) async fn echo_trace_id(
    headers: axum::http::HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    let trace_id = trace_id_from_headers(&headers)
        .map(|trace_id| trace_id.into_uuid().to_string())
        .unwrap_or_else(|| "missing".to_owned());

    (StatusCode::OK, Json(json!({ "trace_id": trace_id })))
}

pub(super) async fn oversized_body_route(body: Bytes) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({ "bytes": body.len() })))
}

pub(super) async fn json_body_route(
    Json(payload): Json<TestJsonPayload>,
) -> Json<serde_json::Value> {
    Json(json!({ "value": payload.value }))
}

pub(super) async fn slow_route() -> (StatusCode, Json<serde_json::Value>) {
    tokio::time::sleep(Duration::from_millis(50)).await;
    (StatusCode::OK, Json(json!({ "status": "slow" })))
}

pub(super) async fn mapped_internal_error_route(
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let request_id = request_id_from_headers(&headers);
    let mapped = TestMappedError::InternalDependencyFailure;

    mapped
        .to_http_error_response()
        .with_optional_request_id(request_id)
}

pub(super) async fn explicit_json_not_found_route() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("Application/Problem+Json; Charset=UTF-8"),
    );

    (
        StatusCode::NOT_FOUND,
        headers,
        Json(json!({
            "error": {
                "code": "explicit_not_found",
                "message": "Explicit not found"
            }
        })),
    )
}

pub(super) async fn custom_correlation_headers_route() -> impl IntoResponse {
    let request_id = RequestId::generate()
        .to_header_value()
        .expect("generated request ID should format as header");
    let trace_id = TraceId::generate()
        .to_header_value()
        .expect("generated trace ID should format as header");
    let mut headers = HeaderMap::new();

    headers.insert(X_REQUEST_ID, request_id);
    headers.insert(X_TRACE_ID, trace_id);

    (StatusCode::NO_CONTENT, headers)
}

pub(super) async fn options_route() -> StatusCode {
    StatusCode::NO_CONTENT
}

#[derive(Debug)]
pub(super) struct BlockedRouteState {
    entered_count: AtomicUsize,
    pub(super) entered: tokio::sync::Notify,
    pub(super) release: tokio::sync::Notify,
}

impl BlockedRouteState {
    pub(super) fn new() -> Self {
        Self {
            entered_count: AtomicUsize::new(0),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        }
    }

    pub(super) fn entered_count(&self) -> usize {
        self.entered_count.load(Ordering::SeqCst)
    }
}

pub(super) async fn blocked_route(
    axum::extract::State(state): axum::extract::State<Arc<BlockedRouteState>>,
) -> StatusCode {
    state.entered_count.fetch_add(1, Ordering::SeqCst);
    state.entered.notify_one();
    state.release.notified().await;

    StatusCode::OK
}

pub(super) async fn failing_route() -> StatusCode {
    StatusCode::INTERNAL_SERVER_ERROR
}
