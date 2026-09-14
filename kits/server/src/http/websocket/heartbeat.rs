// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::error::{
    WebSocketConfigError, WebSocketHeartbeatConfigField, WebSocketValidationErrorReason,
};

/// Conservative default server heartbeat interval.
pub const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
/// Conservative default idle timeout.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
/// Minimum heartbeat interval allowed by the platform.
///
/// Very small intervals can create avoidable timer churn and network noise
/// across many concurrent WebSocket connections.
pub const MINIMUM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);
/// Maximum heartbeat interval allowed by the platform.
///
/// This ceiling keeps dead peer detection from being accidentally configured
/// so high that idle connections linger for operationally surprising periods.
pub const MAXIMUM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(300);
/// Minimum idle timeout allowed by the platform.
///
/// Idle timeout must leave enough time for at least one heartbeat cycle and
/// avoid immediate disconnect churn under ordinary scheduling jitter.
pub const MINIMUM_IDLE_TIMEOUT: Duration = Duration::from_secs(5);
/// Maximum idle timeout allowed by the platform.
///
/// This bounds how long inactive connections can consume runtime resources
/// before the infrastructure layer closes them.
pub const MAXIMUM_IDLE_TIMEOUT: Duration = Duration::from_secs(900);

/// Validated ping heartbeat interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartbeatInterval(Duration);

impl HeartbeatInterval {
    /// Creates a validated heartbeat interval.
    pub fn new(value: Duration) -> Result<Self, WebSocketConfigError> {
        if value.is_zero() {
            return Err(WebSocketConfigError::InvalidHeartbeatConfig {
                field: WebSocketHeartbeatConfigField::HeartbeatInterval,
                reason: WebSocketValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        if value < MINIMUM_HEARTBEAT_INTERVAL {
            return Err(WebSocketConfigError::InvalidHeartbeatConfig {
                field: WebSocketHeartbeatConfigField::HeartbeatInterval,
                reason: WebSocketValidationErrorReason::MustBeAtLeastMinimum,
            });
        }

        if value > MAXIMUM_HEARTBEAT_INTERVAL {
            return Err(WebSocketConfigError::InvalidHeartbeatConfig {
                field: WebSocketHeartbeatConfigField::HeartbeatInterval,
                reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            });
        }

        Ok(Self(value))
    }

    /// Returns the interval as a duration.
    pub const fn as_duration(self) -> Duration {
        self.0
    }

    fn safe_default() -> Self {
        Self(DEFAULT_HEARTBEAT_INTERVAL)
    }
}

/// Validated idle timeout for a WebSocket connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleTimeout(Duration);

impl IdleTimeout {
    /// Creates a validated idle timeout.
    pub fn new(value: Duration) -> Result<Self, WebSocketConfigError> {
        if value.is_zero() {
            return Err(WebSocketConfigError::InvalidHeartbeatConfig {
                field: WebSocketHeartbeatConfigField::IdleTimeout,
                reason: WebSocketValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        if value < MINIMUM_IDLE_TIMEOUT {
            return Err(WebSocketConfigError::InvalidHeartbeatConfig {
                field: WebSocketHeartbeatConfigField::IdleTimeout,
                reason: WebSocketValidationErrorReason::MustBeAtLeastMinimum,
            });
        }

        if value > MAXIMUM_IDLE_TIMEOUT {
            return Err(WebSocketConfigError::InvalidHeartbeatConfig {
                field: WebSocketHeartbeatConfigField::IdleTimeout,
                reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            });
        }

        Ok(Self(value))
    }

    /// Returns the timeout as a duration.
    pub const fn as_duration(self) -> Duration {
        self.0
    }

    fn safe_default() -> Self {
        Self(DEFAULT_IDLE_TIMEOUT)
    }
}

/// Validated heartbeat and idle-timeout configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebSocketHeartbeatConfig {
    heartbeat_interval: HeartbeatInterval,
    idle_timeout: IdleTimeout,
}

impl WebSocketHeartbeatConfig {
    /// Constructs validated heartbeat configuration.
    pub fn new(
        heartbeat_interval: HeartbeatInterval,
        idle_timeout: IdleTimeout,
    ) -> Result<Self, WebSocketConfigError> {
        if idle_timeout.as_duration() <= heartbeat_interval.as_duration() {
            return Err(WebSocketConfigError::InvalidHeartbeatConfig {
                field: WebSocketHeartbeatConfigField::IdleTimeout,
                reason: WebSocketValidationErrorReason::IdleTimeoutMustExceedHeartbeatInterval,
            });
        }

        Ok(Self {
            heartbeat_interval,
            idle_timeout,
        })
    }

    /// Returns a conservative safe default heartbeat configuration.
    pub fn safe_defaults() -> Self {
        Self {
            heartbeat_interval: HeartbeatInterval::safe_default(),
            idle_timeout: IdleTimeout::safe_default(),
        }
    }

    /// Returns the heartbeat interval.
    pub const fn heartbeat_interval(self) -> HeartbeatInterval {
        self.heartbeat_interval
    }

    /// Returns the idle timeout.
    pub const fn idle_timeout(self) -> IdleTimeout {
        self.idle_timeout
    }
}

impl Default for WebSocketHeartbeatConfig {
    fn default() -> Self {
        Self::safe_defaults()
    }
}

#[cfg(test)]
#[path = "heartbeat_tests.rs"]
mod tests;
