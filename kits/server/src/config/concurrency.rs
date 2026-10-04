// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::{ConcurrencyLimitConfigField, ConfigError, ConfigValidationErrorReason};
use thiserror::Error;

/// Default maximum number of concurrent HTTP requests handled by one server process.
///
/// This is intentionally conservative. It prevents accidental unbounded request
/// fan-in while leaving enough headroom for ordinary API workloads. Server
/// compositions can raise or lower it explicitly after load testing.
pub const DEFAULT_HTTP_IN_FLIGHT_REQUEST_LIMIT_VALUE: usize = 4_096;
/// Default maximum number of concurrent gRPC requests handled by one server process.
pub const DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT_VALUE: usize = 4_096;
/// Default maximum number of active WebSocket connections per runtime config.
///
/// WebSocket connections are long-lived and can hold memory for bounded
/// queues, heartbeat state, and app handlers. The default therefore starts
/// lower than HTTP/gRPC request concurrency and should be tuned per server
/// composition after load testing.
pub const DEFAULT_WEBSOCKET_CONNECTION_LIMIT_VALUE: usize = 1_024;
/// Platform maximum for runtime in-flight request limits.
///
/// A single process accepting more concurrent application requests than this
/// usually indicates missing admission control or a need for horizontal scale.
pub const MAX_IN_FLIGHT_REQUEST_LIMIT_VALUE: usize = 65_536;

/// Default HTTP in-flight request limit.
pub const DEFAULT_HTTP_IN_FLIGHT_REQUEST_LIMIT: RuntimeConcurrencyLimit =
    RuntimeConcurrencyLimit::new_unchecked(DEFAULT_HTTP_IN_FLIGHT_REQUEST_LIMIT_VALUE);
/// Default gRPC in-flight request limit.
pub const DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT: RuntimeConcurrencyLimit =
    RuntimeConcurrencyLimit::new_unchecked(DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT_VALUE);
/// Default WebSocket active-connection limit.
pub const DEFAULT_WEBSOCKET_CONNECTION_LIMIT: RuntimeConcurrencyLimit =
    RuntimeConcurrencyLimit::new_unchecked(DEFAULT_WEBSOCKET_CONNECTION_LIMIT_VALUE);
/// Maximum accepted runtime concurrency limit.
pub const MAX_IN_FLIGHT_REQUEST_LIMIT: RuntimeConcurrencyLimit =
    RuntimeConcurrencyLimit::new_unchecked(MAX_IN_FLIGHT_REQUEST_LIMIT_VALUE);

/// Validated TCP connection admission limits for one listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionLimitConfig {
    max_live: RuntimeConcurrencyLimit,
    max_per_source: RuntimeConcurrencyLimit,
}

/// Classified TCP admission limit validation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionLimitErrorReason {
    /// The global connection bound must be positive.
    ZeroGlobal,
    /// The per-source connection bound must be positive.
    ZeroPerSource,
    /// A configured bound exceeds the platform maximum.
    AboveMaximum,
    /// Per-source capacity cannot exceed the listener-wide capacity.
    PerSourceExceedsGlobal,
}

/// Typed validation error for TCP connection admission limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid TCP connection limit: {reason:?}")]
pub struct ConnectionLimitError {
    reason: ConnectionLimitErrorReason,
}

impl ConnectionLimitError {
    /// Returns a stable reason without exposing a raw configuration value.
    pub const fn reason(self) -> ConnectionLimitErrorReason {
        self.reason
    }
}

impl ConnectionLimitConfig {
    /// The global bound remains in force for trusted proxies and local sidecars.
    pub const fn secure_defaults() -> Self {
        Self {
            max_live: RuntimeConcurrencyLimit::new_unchecked(2_048),
            max_per_source: RuntimeConcurrencyLimit::new_unchecked(64),
        }
    }

    /// Constructs bounds with a per-source limit no greater than the global limit.
    pub fn new(max_live: usize, max_per_source: usize) -> Result<Self, ConnectionLimitError> {
        if max_live == 0 {
            return Err(ConnectionLimitError {
                reason: ConnectionLimitErrorReason::ZeroGlobal,
            });
        }
        if max_per_source == 0 {
            return Err(ConnectionLimitError {
                reason: ConnectionLimitErrorReason::ZeroPerSource,
            });
        }
        if max_live > MAX_IN_FLIGHT_REQUEST_LIMIT_VALUE
            || max_per_source > MAX_IN_FLIGHT_REQUEST_LIMIT_VALUE
        {
            return Err(ConnectionLimitError {
                reason: ConnectionLimitErrorReason::AboveMaximum,
            });
        }
        if max_per_source > max_live {
            return Err(ConnectionLimitError {
                reason: ConnectionLimitErrorReason::PerSourceExceedsGlobal,
            });
        }
        Ok(Self {
            max_live: RuntimeConcurrencyLimit::new_unchecked(max_live),
            max_per_source: RuntimeConcurrencyLimit::new_unchecked(max_per_source),
        })
    }

    /// Maximum concurrent TCP connections admitted by the listener.
    pub const fn max_live(self) -> usize {
        self.max_live.as_usize()
    }

    /// Maximum concurrent TCP connections from an untrusted source network.
    pub const fn max_per_source(self) -> usize {
        self.max_per_source.as_usize()
    }
}

/// Validated in-flight request concurrency limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConcurrencyLimit(usize);

/// Runtime resource-limit enforcement model.
///
/// Pingora uses approximate estimators for some high-scale observations. For
/// ReallyMe admission control we intentionally use exact, fail-closed limits:
/// a request or connection is either admitted by a bounded limiter or rejected
/// through a stable public overload response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimitEnforcement {
    /// Exact in-process fail-closed enforcement.
    ExactFailClosed,
}

impl RuntimeConcurrencyLimit {
    /// Constructs a validated runtime concurrency limit.
    pub fn new(value: usize, field: ConcurrencyLimitConfigField) -> Result<Self, ConfigError> {
        if value == 0 {
            return Err(ConfigError::InvalidConcurrencyLimitConfig {
                field,
                reason: ConfigValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        if value > MAX_IN_FLIGHT_REQUEST_LIMIT_VALUE {
            return Err(ConfigError::InvalidConcurrencyLimitConfig {
                field,
                reason: ConfigValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            });
        }

        Ok(Self(value))
    }

    /// Returns the validated concurrency limit.
    pub const fn as_usize(self) -> usize {
        self.0
    }

    /// Returns the enforcement model for this limit.
    pub const fn enforcement(self) -> ResourceLimitEnforcement {
        let _value = self.0;

        ResourceLimitEnforcement::ExactFailClosed
    }

    const fn new_unchecked(value: usize) -> Self {
        Self(value)
    }
}

#[cfg(test)]
#[path = "concurrency_tests.rs"]
mod tests;
