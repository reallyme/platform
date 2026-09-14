// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared runtime configuration primitives.
//!
//! This module contains reusable infrastructure configuration only. It exists
//! to help services load, validate, and hold runtime settings such as bind
//! addresses, timeouts, body limits, observability settings, and generic secret
//! values.
//!
//! Product-specific configuration belongs in app crates. Shared business
//! semantics belong in `reallyme-domain`.

mod body;
mod concurrency;
#[cfg(feature = "http")]
mod cors;
mod defaults;
mod env;
mod environment;
mod error;
#[cfg(feature = "http")]
mod http3;
mod network;
mod observability;
mod secret;
#[cfg(feature = "http")]
mod security;
#[cfg(any(feature = "http", feature = "tonic-grpc"))]
mod servers;
mod timing;

pub use body::{BodyLimitConfig, RequestBodyLimitBytes};
pub use concurrency::{
    DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT, DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT_VALUE,
    DEFAULT_HTTP_IN_FLIGHT_REQUEST_LIMIT, DEFAULT_HTTP_IN_FLIGHT_REQUEST_LIMIT_VALUE,
    DEFAULT_WEBSOCKET_CONNECTION_LIMIT, DEFAULT_WEBSOCKET_CONNECTION_LIMIT_VALUE,
    MAX_IN_FLIGHT_REQUEST_LIMIT, MAX_IN_FLIGHT_REQUEST_LIMIT_VALUE, ResourceLimitEnforcement,
    RuntimeConcurrencyLimit,
};
#[cfg(feature = "http")]
pub use cors::{CorsConfig, ExactCorsOrigin, ExactCorsOrigins};
pub use defaults::{
    DEFAULT_EMIT_SPAN_EVENTS, DEFAULT_LOG_FORMAT, DEFAULT_METRICS_IDLE_TIMEOUT,
    DEFAULT_REQUEST_BODY_LIMIT_BYTES, DEFAULT_REQUEST_TIMEOUT,
};
pub use env::{
    EnvironmentProvider, FromEnvironment, ProcessEnvironment, ValidateConfig, load_env,
    load_from_environment, load_var_with_criticality, optional_var, required_var,
};
pub use environment::ServiceEnvironmentParseError;
pub use environment::{ConfigCriticality, EnvVarName, ServiceEnvironment};
pub use error::{
    BindAddressConfigField, BodyLimitConfigField, ConcurrencyLimitConfigField, ConfigError,
    ConfigValidationErrorReason, ConfigValueErrorReason, CorsConfigField, EnvVarNameErrorReason,
    GrpcServerConfigField, Http3ServerConfigField, HttpServerConfigField, ObservabilityConfigField,
    TimeoutConfigField,
};
#[cfg(feature = "http")]
pub use http3::Http3ServerConfig;
pub use network::{BindAddress, NetworkPort};
pub use observability::{
    HttpRequestLogField, HttpRequestLogFieldParseError, HttpRequestLogFields, HttpRequestLogMode,
    HttpRequestLogModeParseError, HttpRequestLoggingConfig, LogFormat, LogFormatParseError,
    ObservabilityConfig, ObservabilityConfigSummary,
};
pub use secret::{Secret, SecretString};
#[cfg(feature = "http")]
pub use security::{
    DEFAULT_HTTP_HEADER_BYTES_LIMIT, DEFAULT_HTTP_HEADER_BYTES_LIMIT_VALUE,
    DEFAULT_HTTP_HEADER_COUNT_LIMIT, DEFAULT_HTTP_HEADER_COUNT_LIMIT_VALUE,
    ExternalOriginPolicyConfig, HostAuthority, HostAuthorityPolicy, HttpHeaderBytesLimit,
    HttpHeaderCountLimit, HttpHeaderLimitConfig, HttpSecurityConfig,
    MAX_HTTP_HEADER_BYTES_LIMIT_VALUE, MAX_HTTP_HEADER_COUNT_LIMIT_VALUE, OperationalRouteAccess,
    SecurityHeadersConfig, TrustedProxyHeaders, TrustedProxyRange,
    TrustedProxyRequestMetadataConfig,
};
#[cfg(feature = "tonic-grpc")]
pub use servers::{GrpcServerConfig, GrpcServerConfigSummary};
#[cfg(feature = "http")]
pub use servers::{HttpServerConfig, HttpServerConfigSummary};
pub use timing::{MINIMUM_METRICS_IDLE_TIMEOUT, MetricsIdleTimeout, RequestTimeout, TimeoutConfig};

#[cfg(test)]
mod tests;
