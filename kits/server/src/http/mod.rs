// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! HTTP transport primitives and middleware helpers.
//!
//! This module intentionally focuses on reusable transport-layer concerns. The
//! validated runtime configuration types themselves live in `crate::config`,
//! and are re-exported here only for HTTP-focused consumers that prefer a
//! transport-centric import path.
//!
//! # Examples
//!
//! ```rust
//! use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
//! use std::time::Duration;
//!
//! use axum::Router;
//! use reallyme_server_kit::config::{
//!     BindAddress, BodyLimitConfig, CorsConfig, HttpServerConfig, RequestBodyLimitBytes,
//!     RequestTimeout, TimeoutConfig,
//! };
//! use reallyme_server_kit::http::{
//!     apply_standard_router_layers, timeout_layer,
//! };
//!
//! let bind_address =
//!     BindAddress::new(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080)))?;
//! let timeout = TimeoutConfig::new(RequestTimeout::new(Duration::from_secs(2))?);
//! let body_limit = BodyLimitConfig::new(RequestBodyLimitBytes::new(1024 * 1024)?);
//! let config = HttpServerConfig::new(bind_address, CorsConfig::no_cors(), timeout, body_limit);
//!
//! let _router: Router<()> = apply_standard_router_layers(Router::new(), &config);
//!
//! let _timeout = timeout_layer(&config);
//! # Ok::<(), reallyme_server_kit::config::ConfigError>(())
//! ```

mod content_type;
mod cors;
mod error;
mod ids;
mod internal;
mod layers;
mod response;
mod routes;
mod settings;
#[cfg(feature = "testing")]
mod testing;
mod visibility;
#[cfg(feature = "websocket")]
mod websocket;

pub use crate::config::ExactCorsOrigins;
pub use crate::config::{
    HostAuthority, HostAuthorityPolicy, HttpHeaderBytesLimit, HttpHeaderCountLimit,
    HttpHeaderLimitConfig, HttpSecurityConfig, OperationalRouteAccess, SecurityHeadersConfig,
    TrustedProxyHeaders, TrustedProxyRange,
};
pub use crate::transport::{RequestId, TraceId};
pub use cors::{app_cors_layer, cors_layer};
pub use error::{ErrorCode, PublicHttpError, ToHttpErrorResponse};
pub use ids::{
    IdentifierHeaderError, X_REQUEST_ID, X_TRACE_ID, request_id_from_extensions,
    request_id_from_headers, trace_id_from_extensions, trace_id_from_headers,
};
pub use internal::{
    InternalCallCorrelationIds, InternalRequestHeaderError, VerifiedTransportSecurity,
    X_INTERNAL_CALLER, X_SERVICE_TOKEN, attach_internal_request_headers,
    internal_call_correlation_ids_from_extensions,
};
pub use layers::{
    ExternalRequestOrigin, ForwardedClientIp, ForwardedHost, ForwardedProto, body_limit_layer,
    concurrency_limit_layer, default_body_limit, listener_identity_layer,
    normalize_http_error_responses_layer, request_id_layer, route_visibility_layer,
    route_visibility_layer_with_rate_limit_registry, security_layer, timeout_layer, trace_id_layer,
    trace_layer,
};
pub use response::{JsonErrorBody, JsonErrorEnvelope, JsonErrorResponse};
pub use routes::{
    HEALTHZ_PATH, METRICS_PATH, READYZ_PATH, VERSION_PATH, apply_standard_router_layers,
    apply_standard_router_layers_with_request_logging, healthz_route, metrics_route,
    operational_routes, readyz_route, version_route,
};
pub use settings::{
    BodyLimitConfig, CorsConfig, ExactCorsOrigin, HttpServerConfig, RequestBodyLimitBytes,
    RequestTimeout, TimeoutConfig,
};
#[cfg(feature = "testing")]
pub use testing::{
    TestRequest, TestResponse, TestServer, TestServerBuilder, TestServerConfig, Transport,
};
pub use visibility::{
    HttpAuthPolicyName, HttpListenerIdentity, HttpListenerName, HttpListenerVisibility,
    HttpRateLimitTierName, HttpRouteMatchKind, HttpRoutePolicyError, HttpRoutePolicyErrorReason,
    HttpRoutePrefix, HttpRouteVisibility, HttpRouteVisibilityPolicy, HttpRouteVisibilityRule,
    HttpVisibilityClass,
};
#[cfg(feature = "websocket")]
pub use websocket::{
    CloseFrame, CloseGracePeriod, ConnectionId, DEFAULT_CLOSE_GRACE_PERIOD,
    DEFAULT_HEARTBEAT_INTERVAL, DEFAULT_IDLE_TIMEOUT, DEFAULT_INBOUND_FRAME_SIZE_BYTES,
    DEFAULT_INBOUND_MESSAGE_SIZE_BYTES, DEFAULT_OUTBOUND_QUEUE_CAPACITY, HeartbeatInterval,
    IdleTimeout, InboundFrameSizeBytes, InboundMessageSizeBytes, MAX_INBOUND_FRAME_SIZE_BYTES,
    MAX_INBOUND_MESSAGE_SIZE_BYTES, MAX_OUTBOUND_QUEUE_CAPACITY, MAXIMUM_CLOSE_GRACE_PERIOD,
    MAXIMUM_HEARTBEAT_INTERVAL, MAXIMUM_IDLE_TIMEOUT, MINIMUM_CLOSE_GRACE_PERIOD,
    MINIMUM_HEARTBEAT_INTERVAL, MINIMUM_IDLE_TIMEOUT, NoopWebSocketConnectionHooks,
    OutboundQueueCapacity, WebSocket, WebSocketApplicationMessage,
    WebSocketApplicationMessageOwned, WebSocketCloseReason, WebSocketConfigError,
    WebSocketConnectionContext, WebSocketConnectionErrorLabel, WebSocketConnectionHandle,
    WebSocketConnectionHooks, WebSocketConnectionLimitError, WebSocketConnectionLimitErrorReason,
    WebSocketConnectionLimiter, WebSocketConnectionOutcome, WebSocketConnectionOutcomeLabel,
    WebSocketConnectionPermit, WebSocketConnectionRuntime, WebSocketHandlerAction,
    WebSocketHeartbeatConfig, WebSocketHeartbeatConfigField, WebSocketLimitField, WebSocketLimits,
    WebSocketMessage, WebSocketMessageHandler, WebSocketSendError, WebSocketSendErrorReason,
    WebSocketShutdownConfig, WebSocketShutdownConfigField, WebSocketUpgrade,
    WebSocketValidationErrorReason, configure_websocket_upgrade, describe_websocket_metrics,
    gracefully_close_websocket, record_websocket_connection_closed,
    record_websocket_connection_error, record_websocket_connection_opened,
    record_websocket_connection_timeout, run_websocket_connection, websocket_close_code,
    websocket_upgrade_response,
};
