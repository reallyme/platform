// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validated transport deadlines for native gRPC listeners.

use std::time::Duration;

/// Transport deadlines applied to every connection on one gRPC listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrpcTransportTimeouts {
    keepalive_interval: Duration,
    keepalive_timeout: Duration,
    max_age: Duration,
    drain_grace: Duration,
    idle_timeout: Duration,
    first_request_timeout: Duration,
}

/// Finite reason a native gRPC transport deadline is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcTransportTimeoutsErrorReason {
    /// A deadline must be positive to preserve a finite resource bound.
    Zero,
    /// The age and grace cannot be represented as one hard deadline.
    AgeOverflow,
    /// A deadline cannot be scheduled by the runtime clock.
    DeadlineOverflow,
}

/// Invalid native gRPC transport deadline configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid gRPC transport deadline: {reason:?}")]
pub struct GrpcTransportTimeoutsError {
    reason: GrpcTransportTimeoutsErrorReason,
}

impl GrpcTransportTimeoutsError {
    /// Returns the finite configuration failure reason.
    pub const fn reason(self) -> GrpcTransportTimeoutsErrorReason {
        self.reason
    }
}

impl GrpcTransportTimeouts {
    /// Constructs finite transport deadlines for one listener.
    pub fn new(
        keepalive_interval: Duration,
        keepalive_timeout: Duration,
        max_age: Duration,
        drain_grace: Duration,
        idle_timeout: Duration,
        first_request_timeout: Duration,
    ) -> Result<Self, GrpcTransportTimeoutsError> {
        if [
            keepalive_interval,
            keepalive_timeout,
            max_age,
            drain_grace,
            idle_timeout,
            first_request_timeout,
        ]
        .contains(&Duration::ZERO)
        {
            return Err(GrpcTransportTimeoutsError {
                reason: GrpcTransportTimeoutsErrorReason::Zero,
            });
        }
        let hard_cap = max_age
            .checked_add(drain_grace)
            .ok_or(GrpcTransportTimeoutsError {
                reason: GrpcTransportTimeoutsErrorReason::AgeOverflow,
            })?;
        let now = tokio::time::Instant::now();
        for duration in [
            keepalive_interval,
            keepalive_timeout,
            hard_cap,
            idle_timeout,
            first_request_timeout,
        ] {
            now.checked_add(duration)
                .ok_or(GrpcTransportTimeoutsError {
                    reason: GrpcTransportTimeoutsErrorReason::DeadlineOverflow,
                })?;
        }
        Ok(Self {
            keepalive_interval,
            keepalive_timeout,
            max_age,
            drain_grace,
            idle_timeout,
            first_request_timeout,
        })
    }

    pub(crate) const fn keepalive_interval(self) -> Duration {
        self.keepalive_interval
    }

    pub(crate) const fn keepalive_timeout(self) -> Duration {
        self.keepalive_timeout
    }

    pub(crate) const fn max_age(self) -> Duration {
        self.max_age
    }

    pub(crate) const fn drain_grace(self) -> Duration {
        self.drain_grace
    }

    pub(crate) fn hard_cap(self) -> Duration {
        // The constructor validates this sum, including for the default.
        self.max_age.saturating_add(self.drain_grace)
    }

    pub(crate) const fn idle_timeout(self) -> Duration {
        self.idle_timeout
    }

    pub(crate) const fn first_request_timeout(self) -> Duration {
        self.first_request_timeout
    }
}

impl Default for GrpcTransportTimeouts {
    fn default() -> Self {
        Self {
            keepalive_interval: Duration::from_secs(30),
            keepalive_timeout: Duration::from_secs(10),
            max_age: Duration::from_secs(60),
            drain_grace: Duration::from_secs(600),
            idle_timeout: Duration::from_secs(120),
            first_request_timeout: Duration::from_secs(5),
        }
    }
}

#[cfg(test)]
#[path = "transport_timeouts_tests.rs"]
mod tests;
