// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validated HTTP transport lifetime and progress deadlines.

use std::time::Duration;

use thiserror::Error;

const MAX_TIMEOUT: Duration = Duration::from_secs(86_400);

/// Validated deadlines for HTTP connection lifetime and request progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpTransportTimeouts {
    max_connection_age: Duration,
    drain_grace: Duration,
    idle: Duration,
    write_stall: Duration,
}

/// Stable reason for rejecting an HTTP transport deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpTransportTimeoutErrorReason {
    /// A zero deadline would disable the corresponding guard.
    ZeroDuration,
    /// The deadline exceeds the operational upper bound.
    AboveMaximum,
}

/// Typed HTTP transport timeout validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid HTTP transport timeout: {reason:?}")]
pub struct HttpTransportTimeoutError {
    reason: HttpTransportTimeoutErrorReason,
}

impl HttpTransportTimeoutError {
    /// Returns the stable validation reason without retaining configuration values.
    pub const fn reason(self) -> HttpTransportTimeoutErrorReason {
        self.reason
    }
}

impl HttpTransportTimeouts {
    /// Returns conservative deadlines for an uncustomized listener.
    pub const fn secure_defaults() -> Self {
        Self {
            max_connection_age: Duration::from_secs(600),
            drain_grace: Duration::from_secs(30),
            idle: Duration::from_secs(60),
            write_stall: Duration::from_secs(30),
        }
    }

    /// Validates bounded, nonzero deadlines for every transport phase.
    pub fn new(
        max_connection_age: Duration,
        drain_grace: Duration,
        idle: Duration,
        write_stall: Duration,
    ) -> Result<Self, HttpTransportTimeoutError> {
        for duration in [max_connection_age, drain_grace, idle, write_stall] {
            if duration.is_zero() {
                return Err(HttpTransportTimeoutError {
                    reason: HttpTransportTimeoutErrorReason::ZeroDuration,
                });
            }
            if duration > MAX_TIMEOUT {
                return Err(HttpTransportTimeoutError {
                    reason: HttpTransportTimeoutErrorReason::AboveMaximum,
                });
            }
        }
        Ok(Self {
            max_connection_age,
            drain_grace,
            idle,
            write_stall,
        })
    }

    /// Returns the maximum connection lifetime.
    pub const fn max_connection_age(self) -> Duration {
        self.max_connection_age
    }

    /// Returns the graceful drain deadline.
    pub const fn drain_grace(self) -> Duration {
        self.drain_grace
    }

    /// Returns the connection idle deadline. A completed response may retain
    /// the socket for the longer write-stall allowance while bytes drain.
    pub const fn idle(self) -> Duration {
        self.idle
    }

    /// Returns the response write-progress deadline.
    pub const fn write_stall(self) -> Duration {
        self.write_stall
    }
}

#[cfg(test)]
mod tests;
