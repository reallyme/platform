// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use thiserror::Error;

const DEFAULT_READINESS_DRAIN_DELAY: Duration = Duration::from_secs(2);
const MAX_READINESS_DRAIN_DELAY: Duration = Duration::from_secs(30);

/// Time allowed for traffic routers to observe NotReady before listeners drain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadinessDrainDelay(Duration);

/// Invalid readiness propagation delay reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadinessDrainDelayErrorReason {
    /// The requested delay exceeds the bounded propagation window.
    ExceedsMaximum,
}

/// Typed readiness propagation delay validation failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("invalid readiness drain delay")]
pub struct ReadinessDrainDelayError {
    reason: ReadinessDrainDelayErrorReason,
}

impl ReadinessDrainDelayError {
    /// Returns the stable validation reason.
    pub const fn reason(self) -> ReadinessDrainDelayErrorReason {
        self.reason
    }
}

impl ReadinessDrainDelay {
    /// Validates a readiness propagation window. Zero disables the delay.
    pub fn new(value: Duration) -> Result<Self, ReadinessDrainDelayError> {
        if value > MAX_READINESS_DRAIN_DELAY {
            return Err(ReadinessDrainDelayError {
                reason: ReadinessDrainDelayErrorReason::ExceedsMaximum,
            });
        }
        Ok(Self(value))
    }

    /// Returns the configured propagation window.
    pub const fn as_duration(self) -> Duration {
        self.0
    }
}

impl Default for ReadinessDrainDelay {
    fn default() -> Self {
        Self(DEFAULT_READINESS_DRAIN_DELAY)
    }
}

#[cfg(test)]
#[path = "readiness_drain_tests.rs"]
mod tests;
