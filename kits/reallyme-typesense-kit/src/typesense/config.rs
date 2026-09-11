// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typesense connection configuration.

use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use url::Url;

/// Validated Typesense HTTP endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypesenseEndpoint(String);

impl TypesenseEndpoint {
    /// Parses and normalizes a Typesense endpoint.
    pub fn parse(raw: impl Into<String>) -> Result<Self, TypesenseConfigError> {
        let value = raw.into();
        let trimmed = value.trim().trim_end_matches('/');

        if trimmed.is_empty() {
            return Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::EmptyEndpoint,
            });
        }

        let parsed = Url::parse(trimmed).map_err(|_| TypesenseConfigError::Invalid {
            reason: TypesenseConfigErrorReason::MalformedEndpoint,
        })?;

        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::EndpointContainsUserInfo,
            });
        }

        if !(parsed.scheme() == "http" || parsed.scheme() == "https") {
            return Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::UnsupportedEndpointScheme,
            });
        }

        if parsed.host().is_none() {
            return Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::MalformedEndpoint,
            });
        }

        if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::EndpointContainsPath,
            });
        }

        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the normalized endpoint string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Connection and request defaults for Typesense.
#[derive(Clone)]
pub struct TypesenseConfig {
    /// Candidate Typesense endpoints in preferred order (nearest first).
    endpoints: Vec<TypesenseEndpoint>,
    /// Endpoint selection and failover policy.
    endpoint_selection: TypesenseEndpointSelection,
    /// API key used for the connector. This value must not be logged.
    api_key: SecretString,
    /// Base request timeout for search and mutation traffic.
    request_timeout: Duration,
    /// Optional timeout override for bulk import requests.
    import_request_timeout: Option<Duration>,
    /// Maximum number of retries for transient failures.
    max_retries: u8,
    /// Initial backoff duration before the first retry.
    retry_initial_delay: Duration,
    /// Maximum backoff duration for exponential backoff.
    retry_max_delay: Duration,
    /// Jitter percentage applied on top of exponential backoff.
    retry_jitter_percent: u8,
    /// How long idle connections remain in the pool.
    pool_idle_timeout: Option<Duration>,
    /// TCP keepalive interval.
    tcp_keepalive: Option<Duration>,
    /// Whether to use HTTP/2 prior knowledge for endpoint negotiation.
    http2_prior_knowledge: bool,
    /// Whether to enable HTTP/2 adaptive flow control.
    http2_adaptive_window: bool,
    /// HTTP/2 keep-alive interval.
    http2_keep_alive_interval: Option<Duration>,
    /// Whether to enable gzip request/response compression.
    use_gzip: bool,
}

impl TypesenseConfig {
    /// Creates a config from already-validated parts.
    ///
    /// The first endpoint in the list is treated as the preferred primary.
    pub fn new(
        endpoint: TypesenseEndpoint,
        api_key: SecretString,
        request_timeout: Duration,
    ) -> Result<Self, TypesenseConfigError> {
        if api_key.expose_secret().is_empty() {
            return Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::EmptyApiKey,
            });
        }

        if request_timeout.is_zero() {
            return Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::ZeroRequestTimeout,
            });
        }

        Self::new_with_endpoints(vec![endpoint], api_key, request_timeout)
    }

    /// Creates a config from validated endpoints and request policy.
    pub fn new_with_endpoints(
        endpoints: Vec<TypesenseEndpoint>,
        api_key: SecretString,
        request_timeout: Duration,
    ) -> Result<Self, TypesenseConfigError> {
        if endpoints.is_empty() {
            return Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::EmptyEndpointList,
            });
        }

        if api_key.expose_secret().is_empty() {
            return Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::EmptyApiKey,
            });
        }

        if request_timeout.is_zero() {
            return Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::ZeroRequestTimeout,
            });
        }

        Ok(Self {
            endpoints,
            endpoint_selection: TypesenseEndpointSelection::NearestNode,
            api_key,
            request_timeout,
            import_request_timeout: None,
            max_retries: 3,
            retry_initial_delay: Duration::from_millis(100),
            retry_max_delay: Duration::from_secs(2),
            retry_jitter_percent: 25,
            pool_idle_timeout: None,
            tcp_keepalive: None,
            http2_prior_knowledge: false,
            http2_adaptive_window: false,
            http2_keep_alive_interval: None,
            use_gzip: false,
        })
    }

    /// Sets the endpoint selection strategy.
    #[must_use]
    pub fn with_endpoint_selection(
        mut self,
        endpoint_selection: TypesenseEndpointSelection,
    ) -> Self {
        self.endpoint_selection = endpoint_selection;
        self
    }

    /// Replaces the endpoint list with validated candidates.
    ///
    /// If `endpoints` is empty, this keeps the current list unchanged.
    #[must_use]
    pub fn with_endpoints(mut self, endpoints: Vec<TypesenseEndpoint>) -> Self {
        if !endpoints.is_empty() {
            self.endpoints = endpoints;
        }
        self
    }

    /// Sets the import-request timeout.
    ///
    /// A zero-duration value is ignored and clears override behavior.
    #[must_use]
    pub fn with_import_request_timeout(mut self, import_request_timeout: Option<Duration>) -> Self {
        if let Some(value) = import_request_timeout {
            if !value.is_zero() {
                self.import_request_timeout = Some(value);
            }
        } else {
            self.import_request_timeout = None;
        }
        self
    }

    /// Sets the maximum number of retries for transient upstream and transport faults.
    #[must_use]
    pub fn with_max_retries(mut self, max_retries: u8) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Sets the initial backoff delay used in retry calculations.
    #[must_use]
    pub fn with_retry_initial_delay(mut self, retry_initial_delay: Duration) -> Self {
        self.retry_initial_delay = retry_initial_delay;
        self
    }

    /// Sets the upper bound for retry backoff delays.
    #[must_use]
    pub fn with_retry_max_delay(mut self, retry_max_delay: Duration) -> Self {
        self.retry_max_delay = retry_max_delay;
        self
    }

    /// Sets the jitter percentage for retry delays.
    #[must_use]
    pub fn with_retry_jitter_percent(mut self, retry_jitter_percent: u8) -> Self {
        self.retry_jitter_percent = retry_jitter_percent.min(100);
        self
    }

    /// Sets the pool idle timeout.
    #[must_use]
    pub const fn with_pool_idle_timeout(mut self, pool_idle_timeout: Option<Duration>) -> Self {
        self.pool_idle_timeout = pool_idle_timeout;
        self
    }

    /// Sets the TCP keepalive interval.
    #[must_use]
    pub const fn with_tcp_keepalive(mut self, tcp_keepalive: Option<Duration>) -> Self {
        self.tcp_keepalive = tcp_keepalive;
        self
    }

    /// Enables HTTP/2 prior knowledge.
    #[must_use]
    pub const fn with_http2_prior_knowledge(mut self, http2_prior_knowledge: bool) -> Self {
        self.http2_prior_knowledge = http2_prior_knowledge;
        self
    }

    /// Enables HTTP/2 adaptive windowing.
    #[must_use]
    pub const fn with_http2_adaptive_window(mut self, http2_adaptive_window: bool) -> Self {
        self.http2_adaptive_window = http2_adaptive_window;
        self
    }

    /// Sets the HTTP/2 keep-alive interval.
    #[must_use]
    pub const fn with_http2_keep_alive_interval(
        mut self,
        http2_keep_alive_interval: Option<Duration>,
    ) -> Self {
        self.http2_keep_alive_interval = http2_keep_alive_interval;
        self
    }

    /// Enables gzip transport compression.
    #[must_use]
    pub const fn with_gzip(mut self, use_gzip: bool) -> Self {
        self.use_gzip = use_gzip;
        self
    }

    /// Returns the configured endpoint selection strategy.
    #[must_use]
    pub const fn endpoint_selection(&self) -> TypesenseEndpointSelection {
        self.endpoint_selection
    }

    /// Returns all configured endpoints.
    #[must_use]
    pub fn endpoints(&self) -> &[TypesenseEndpoint] {
        &self.endpoints
    }

    /// Returns the API key secret wrapper.
    #[must_use]
    pub const fn api_key(&self) -> &SecretString {
        &self.api_key
    }

    /// Returns the configured request timeout used for search and mutation calls.
    #[must_use]
    pub const fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    /// Returns the configured import timeout override.
    #[must_use]
    pub const fn import_request_timeout(&self) -> Option<Duration> {
        self.import_request_timeout
    }

    /// Returns the configured maximum retries.
    #[must_use]
    pub const fn max_retries(&self) -> u8 {
        self.max_retries
    }

    /// Returns the configured initial retry backoff.
    #[must_use]
    pub const fn retry_initial_delay(&self) -> Duration {
        self.retry_initial_delay
    }

    /// Returns the configured maximum retry backoff.
    #[must_use]
    pub const fn retry_max_delay(&self) -> Duration {
        self.retry_max_delay
    }

    /// Returns the configured jitter percentage for retries.
    #[must_use]
    pub const fn retry_jitter_percent(&self) -> u8 {
        self.retry_jitter_percent
    }

    /// Returns the configured pool idle timeout.
    #[must_use]
    pub const fn pool_idle_timeout(&self) -> Option<Duration> {
        self.pool_idle_timeout
    }

    /// Returns the configured TCP keepalive interval.
    #[must_use]
    pub const fn tcp_keepalive(&self) -> Option<Duration> {
        self.tcp_keepalive
    }

    /// Returns whether HTTP/2 prior knowledge is enabled.
    #[must_use]
    pub const fn http2_prior_knowledge(&self) -> bool {
        self.http2_prior_knowledge
    }

    /// Returns whether HTTP/2 adaptive windowing is enabled.
    #[must_use]
    pub const fn http2_adaptive_window(&self) -> bool {
        self.http2_adaptive_window
    }

    /// Returns the configured HTTP/2 keep-alive interval.
    #[must_use]
    pub const fn http2_keep_alive_interval(&self) -> Option<Duration> {
        self.http2_keep_alive_interval
    }

    /// Returns whether gzip is enabled.
    #[must_use]
    pub const fn use_gzip(&self) -> bool {
        self.use_gzip
    }
}

/// Configuration parsing error.
#[derive(Debug, Error)]
pub enum TypesenseConfigError {
    /// A config value failed validation.
    #[error("invalid typesense configuration")]
    Invalid {
        /// Stable machine-readable reason.
        reason: TypesenseConfigErrorReason,
    },
}

/// Stable reason for configuration validation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypesenseConfigErrorReason {
    /// The endpoint was empty.
    EmptyEndpoint,
    /// The endpoint did not use an HTTP scheme.
    UnsupportedEndpointScheme,
    /// The endpoint failed URL parsing.
    MalformedEndpoint,
    /// The endpoint included embedded credentials in a userinfo segment.
    EndpointContainsUserInfo,
    /// The endpoint included a path component.
    EndpointContainsPath,
    /// The endpoint list was empty.
    EmptyEndpointList,
    /// The API key was empty.
    EmptyApiKey,
    /// The request timeout was zero.
    ZeroRequestTimeout,
}

/// Endpoint selection and failover behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypesenseEndpointSelection {
    /// Use the configured primary endpoint as-is.
    NearestNode,
    /// Rotate the primary candidate per request.
    RoundRobin,
    /// Retry sequentially across endpoints on retry.
    Failover,
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
