// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::error::{
    ConfigError, ConfigValidationErrorReason, ObservabilityConfigField, TimeoutConfigField,
};

/// Minimum metrics idle timeout to avoid excessive metric series churn.
pub const MINIMUM_METRICS_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// Validated per-request timeout.
///
/// The shared type currently enforces only the cross-service invariant that a
/// timeout must be non-zero. It does not impose a global upper bound yet
/// because acceptable ceilings can vary by service and endpoint. If the
/// platform later adopts a shared maximum, that should be added here
/// deliberately as a documented operational policy.
///
/// # Examples
///
/// ```rust
/// use std::time::Duration;
///
/// use reallyme_server_kit::config::{RequestTimeout, TimeoutConfig};
///
/// let request_timeout = RequestTimeout::new(Duration::from_secs(2))?;
/// let timeout_config = TimeoutConfig::new(request_timeout);
///
/// assert_eq!(
///     timeout_config.request_timeout().as_duration(),
///     Duration::from_secs(2)
/// );
/// # Ok::<(), reallyme_server_kit::config::ConfigError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestTimeout(Duration);

impl RequestTimeout {
    /// Constructs a validated request timeout.
    pub fn new(value: Duration) -> Result<Self, ConfigError> {
        if value.is_zero() {
            return Err(ConfigError::InvalidTimeoutConfig {
                field: TimeoutConfigField::RequestTimeout,
                reason: ConfigValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        Ok(Self(value))
    }

    /// Returns the timeout as a `Duration`.
    pub fn as_duration(self) -> Duration {
        self.0
    }
}

/// Validated metrics idle timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricsIdleTimeout(Duration);

impl MetricsIdleTimeout {
    /// Constructs a validated metrics idle timeout.
    pub fn new(value: Duration) -> Result<Self, ConfigError> {
        if value < MINIMUM_METRICS_IDLE_TIMEOUT {
            return Err(ConfigError::InvalidObservabilityConfig {
                field: ObservabilityConfigField::MetricsIdleTimeout,
                reason: ConfigValidationErrorReason::MustBeAtLeastMinimum,
            });
        }

        Ok(Self(value))
    }

    /// Returns the timeout as a duration.
    pub fn as_duration(self) -> Duration {
        self.0
    }
}

/// Shared timeout configuration used by runtime middleware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutConfig {
    request_timeout: RequestTimeout,
}

impl TimeoutConfig {
    /// Constructs validated timeout configuration.
    pub fn new(request_timeout: RequestTimeout) -> Self {
        Self { request_timeout }
    }

    /// Returns the validated request timeout.
    pub fn request_timeout(&self) -> RequestTimeout {
        self.request_timeout
    }
}
