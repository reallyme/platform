// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

use super::environment::{EnvVarName, ServiceEnvironment};

/// Typed configuration loading and validation failures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// A service provided an invalid environment variable name literal.
    #[error("environment variable name is invalid")]
    InvalidEnvVarName {
        /// Why the literal is invalid.
        reason: EnvVarNameErrorReason,
    },
    /// A required variable is not present in the environment.
    #[error("required environment variable is missing")]
    MissingRequiredVariable {
        /// Name of the missing environment variable.
        name: EnvVarName,
    },
    /// The variable exists but cannot be represented as valid UTF-8 text.
    #[error("environment variable is not valid unicode")]
    InvalidUnicode {
        /// Name of the non-Unicode environment variable.
        name: EnvVarName,
    },
    /// The variable exists but does not satisfy the expected parser.
    #[error("environment variable has an invalid value")]
    InvalidValue {
        /// Name of the invalid environment variable.
        name: EnvVarName,
        /// Why the value is invalid.
        reason: ConfigValueErrorReason,
    },
    /// A production-critical variable was omitted in a production-like
    /// environment.
    #[error("production-critical environment variable is missing")]
    MissingProductionCriticalVariable {
        /// Name of the missing environment variable.
        name: EnvVarName,
        /// Environment in which it was required.
        service_environment: ServiceEnvironment,
    },
    /// Bind-address configuration is invalid.
    #[error("bind address configuration is invalid")]
    InvalidBindAddressConfig {
        /// The invalid bind-address field.
        field: BindAddressConfigField,
        /// Why the field is invalid.
        reason: ConfigValidationErrorReason,
    },
    /// Timeout configuration is invalid.
    #[error("timeout configuration is invalid")]
    InvalidTimeoutConfig {
        /// The invalid timeout field.
        field: TimeoutConfigField,
        /// Why the field is invalid.
        reason: ConfigValidationErrorReason,
    },
    /// Body-limit configuration is invalid.
    #[error("body limit configuration is invalid")]
    InvalidBodyLimitConfig {
        /// The invalid body-limit field.
        field: BodyLimitConfigField,
        /// Why the field is invalid.
        reason: ConfigValidationErrorReason,
    },
    /// Runtime concurrency-limit configuration is invalid.
    #[error("concurrency limit configuration is invalid")]
    InvalidConcurrencyLimitConfig {
        /// The invalid concurrency-limit field.
        field: ConcurrencyLimitConfigField,
        /// Why the field is invalid.
        reason: ConfigValidationErrorReason,
    },
    /// CORS configuration is invalid.
    #[error("cors configuration is invalid")]
    InvalidCorsConfig {
        /// The invalid CORS field.
        field: CorsConfigField,
        /// Why the field is invalid.
        reason: ConfigValidationErrorReason,
    },
    /// CORS policy is not allowed in the configured service environment.
    #[error("cors policy is not allowed in the configured environment")]
    CorsPolicyDisallowedInEnvironment {
        /// The service environment that rejects this CORS policy.
        service_environment: ServiceEnvironment,
    },
    /// HTTP server configuration is invalid.
    #[error("http server configuration is invalid")]
    InvalidHttpServerConfig {
        /// The invalid HTTP field.
        field: HttpServerConfigField,
        /// Why the field is invalid.
        reason: ConfigValidationErrorReason,
    },
    /// HTTP/3 server configuration is invalid.
    #[error("http3 server configuration is invalid")]
    InvalidHttp3ServerConfig {
        /// The invalid HTTP/3 field.
        field: Http3ServerConfigField,
        /// Why the field is invalid.
        reason: ConfigValidationErrorReason,
    },
    /// gRPC server configuration is invalid.
    #[error("grpc server configuration is invalid")]
    InvalidGrpcServerConfig {
        /// The invalid gRPC field.
        field: GrpcServerConfigField,
        /// Why the field is invalid.
        reason: ConfigValidationErrorReason,
    },
    /// Observability configuration is invalid.
    #[error("observability configuration is invalid")]
    InvalidObservabilityConfig {
        /// The invalid observability field.
        field: ObservabilityConfigField,
        /// Why the field is invalid.
        reason: ConfigValidationErrorReason,
    },
}

/// Why an environment variable name literal is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvVarNameErrorReason {
    /// The name is empty.
    Empty,
    /// The name contains unsupported characters.
    InvalidCharacters,
}

/// Why an environment-derived value could not be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigValueErrorReason {
    /// The value is not a valid socket address.
    InvalidSocketAddress,
    /// The value is not a valid integer.
    InvalidInteger,
    /// The value is not a valid service environment.
    InvalidServiceEnvironment,
    /// The value is not a supported log format.
    InvalidLogFormat,
    /// The value is not a valid boolean.
    InvalidBoolean,
}

/// Why a validated config field is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigValidationErrorReason {
    /// The field must be greater than zero.
    MustBeGreaterThanZero,
    /// The field must be at least the configured minimum.
    MustBeAtLeastMinimum,
    /// The field must be less than or equal to the configured maximum.
    MustBeLessThanOrEqualToMaximum,
    /// The field must not be empty.
    MustBeNonEmpty,
    /// The field must be a valid header value.
    InvalidHeaderValue,
    /// The field must be a syntactically valid origin.
    InvalidOrigin,
    /// The configured protocol is recognized but not supported by this build.
    UnsupportedProtocol,
    /// The field must be a valid IP address or CIDR range.
    InvalidNetworkRange,
}

/// Bind-address config fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindAddressConfigField {
    /// The TCP port number.
    Port,
}

/// Timeout config fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutConfigField {
    /// The per-request timeout.
    RequestTimeout,
}

/// Body-limit config fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyLimitConfigField {
    /// The maximum request body size in bytes.
    RequestBodyLimitBytes,
}

/// Concurrency-limit config fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcurrencyLimitConfigField {
    /// Maximum concurrent HTTP requests.
    HttpInFlightRequests,
    /// Maximum concurrent gRPC requests.
    GrpcInFlightRequests,
    /// Maximum concurrent WebSocket connections.
    WebSocketConnections,
}

/// CORS config fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorsConfigField {
    /// The allowed origin value.
    AllowOrigin,
}

/// HTTP config fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpServerConfigField {
    /// The server bind address.
    BindAddress,
    /// Generic HTTP security posture.
    Security,
}

/// HTTP/3 config fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Http3ServerConfigField {
    /// Whether HTTP/3 over QUIC is enabled.
    Enabled,
}

/// gRPC config fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcServerConfigField {
    /// The server bind address.
    BindAddress,
}

/// Observability config fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservabilityConfigField {
    /// The tracing filter directives.
    TracingFilterDirectives,
    /// The metrics idle timeout.
    MetricsIdleTimeout,
    /// HTTP request log sample rate.
    HttpRequestLogSampleRate,
    /// HTTP slow request threshold.
    HttpSlowRequestThreshold,
}
