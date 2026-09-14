// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(feature = "http")]
use super::body::BodyLimitConfig;
#[cfg(feature = "tonic-grpc")]
use super::concurrency::DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT;
#[cfg(feature = "http")]
use super::concurrency::DEFAULT_HTTP_IN_FLIGHT_REQUEST_LIMIT;
use super::concurrency::RuntimeConcurrencyLimit;
#[cfg(feature = "http")]
use super::cors::CorsConfig;
#[cfg(feature = "http")]
use super::http3::Http3ServerConfig;
use super::network::BindAddress;
#[cfg(feature = "http")]
use super::security::HttpSecurityConfig;
#[cfg(feature = "http")]
use super::timing::TimeoutConfig;

/// Shared HTTP server runtime configuration.
#[cfg(feature = "http")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpServerConfig {
    bind_address: BindAddress,
    cors: CorsConfig,
    timeout: TimeoutConfig,
    body_limit: BodyLimitConfig,
    concurrency_limit: RuntimeConcurrencyLimit,
    http3: Http3ServerConfig,
    security: HttpSecurityConfig,
}

#[cfg(feature = "http")]
impl HttpServerConfig {
    /// Constructs validated HTTP server configuration.
    pub fn new(
        bind_address: BindAddress,
        cors: CorsConfig,
        timeout: TimeoutConfig,
        body_limit: BodyLimitConfig,
    ) -> Self {
        Self {
            bind_address,
            cors,
            timeout,
            body_limit,
            concurrency_limit: DEFAULT_HTTP_IN_FLIGHT_REQUEST_LIMIT,
            http3: Http3ServerConfig::disabled(),
            security: HttpSecurityConfig::secure_defaults(),
        }
    }

    /// Constructs HTTP server configuration with an explicit in-flight request limit.
    pub fn with_concurrency_limit(
        bind_address: BindAddress,
        cors: CorsConfig,
        timeout: TimeoutConfig,
        body_limit: BodyLimitConfig,
        concurrency_limit: RuntimeConcurrencyLimit,
    ) -> Self {
        Self {
            bind_address,
            cors,
            timeout,
            body_limit,
            concurrency_limit,
            http3: Http3ServerConfig::disabled(),
            security: HttpSecurityConfig::secure_defaults(),
        }
    }

    /// Returns a copy of this config with explicit HTTP/3 posture.
    ///
    /// The current server kit only supports the disabled HTTP/3 posture. This
    /// method exists so server composition can validate and carry that posture
    /// explicitly without pretending the TCP Axum listener serves QUIC.
    pub fn with_http3_config(mut self, http3: Http3ServerConfig) -> Self {
        self.http3 = http3;
        self
    }

    /// Returns a copy of this config with explicit HTTP security posture.
    pub fn with_security_config(mut self, security: HttpSecurityConfig) -> Self {
        self.security = security;
        self
    }

    /// Returns the validated bind address.
    pub fn bind_address(&self) -> BindAddress {
        self.bind_address
    }

    /// Returns the shared CORS configuration.
    pub fn cors(&self) -> &CorsConfig {
        &self.cors
    }

    /// Returns the shared timeout configuration.
    pub fn timeout(&self) -> TimeoutConfig {
        self.timeout
    }

    /// Returns the shared body-limit configuration.
    pub fn body_limit(&self) -> BodyLimitConfig {
        self.body_limit
    }

    /// Returns the validated in-flight request limit.
    pub fn concurrency_limit(&self) -> RuntimeConcurrencyLimit {
        self.concurrency_limit
    }

    /// Returns the explicit HTTP/3 posture.
    pub fn http3(&self) -> Http3ServerConfig {
        self.http3
    }

    /// Returns the HTTP security posture.
    pub fn security(&self) -> &HttpSecurityConfig {
        &self.security
    }

    /// Returns a startup-safe configuration summary.
    pub fn startup_summary(&self) -> HttpServerConfigSummary {
        HttpServerConfigSummary {
            bind_address: self.bind_address,
            timeout: self.timeout,
            body_limit: self.body_limit,
            concurrency_limit: self.concurrency_limit,
            http3: self.http3,
            security: self.security.clone(),
            // The summary intentionally reuses the validated CORS policy rather
            // than re-rendering it as ad hoc strings. `CorsConfig` itself owns
            // the logging safety policy for exact origins.
            cors: self.cors.clone(),
        }
    }
}

/// Shared gRPC server runtime configuration.
#[cfg(feature = "tonic-grpc")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrpcServerConfig {
    bind_address: BindAddress,
    concurrency_limit: RuntimeConcurrencyLimit,
}

#[cfg(feature = "tonic-grpc")]
impl GrpcServerConfig {
    /// Constructs validated gRPC server configuration.
    pub fn new(bind_address: BindAddress) -> Self {
        Self {
            bind_address,
            concurrency_limit: DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT,
        }
    }

    /// Constructs gRPC server configuration with an explicit in-flight request limit.
    pub fn with_concurrency_limit(
        bind_address: BindAddress,
        concurrency_limit: RuntimeConcurrencyLimit,
    ) -> Self {
        Self {
            bind_address,
            concurrency_limit,
        }
    }

    /// Returns the validated bind address.
    pub fn bind_address(&self) -> BindAddress {
        self.bind_address
    }

    /// Returns the validated in-flight request limit.
    pub fn concurrency_limit(&self) -> RuntimeConcurrencyLimit {
        self.concurrency_limit
    }

    /// Returns a startup-safe configuration summary.
    pub fn startup_summary(&self) -> GrpcServerConfigSummary {
        GrpcServerConfigSummary {
            bind_address: self.bind_address,
            concurrency_limit: self.concurrency_limit,
        }
    }
}

/// Safe startup summary for HTTP server configuration.
#[cfg(feature = "http")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpServerConfigSummary {
    bind_address: BindAddress,
    timeout: TimeoutConfig,
    body_limit: BodyLimitConfig,
    cors: CorsConfig,
    concurrency_limit: RuntimeConcurrencyLimit,
    http3: Http3ServerConfig,
    security: HttpSecurityConfig,
}

#[cfg(feature = "http")]
impl HttpServerConfigSummary {
    /// Returns the validated bind address.
    pub fn bind_address(&self) -> BindAddress {
        self.bind_address
    }

    /// Returns the shared timeout configuration.
    pub fn timeout(&self) -> TimeoutConfig {
        self.timeout
    }

    /// Returns the shared body-limit configuration.
    pub fn body_limit(&self) -> BodyLimitConfig {
        self.body_limit
    }

    /// Returns the shared CORS configuration.
    pub fn cors(&self) -> &CorsConfig {
        &self.cors
    }

    /// Returns the validated in-flight request limit.
    pub fn concurrency_limit(&self) -> RuntimeConcurrencyLimit {
        self.concurrency_limit
    }

    /// Returns the explicit HTTP/3 posture.
    pub fn http3(&self) -> Http3ServerConfig {
        self.http3
    }

    /// Returns the HTTP security posture.
    pub fn security(&self) -> &HttpSecurityConfig {
        &self.security
    }
}

/// Safe startup summary for gRPC server configuration.
#[cfg(feature = "tonic-grpc")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrpcServerConfigSummary {
    bind_address: BindAddress,
    concurrency_limit: RuntimeConcurrencyLimit,
}

#[cfg(feature = "tonic-grpc")]
impl GrpcServerConfigSummary {
    /// Returns the validated bind address.
    pub fn bind_address(&self) -> BindAddress {
        self.bind_address
    }

    /// Returns the validated in-flight request limit.
    pub fn concurrency_limit(&self) -> RuntimeConcurrencyLimit {
        self.concurrency_limit
    }
}
