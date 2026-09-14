// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::{ConcurrencyLimitConfigField, ConfigError, ConfigValidationErrorReason};

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
