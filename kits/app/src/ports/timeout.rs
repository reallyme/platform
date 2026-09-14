// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use thiserror::Error;

/// Host-neutral downstream call timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppPortTimeout(Duration);

impl AppPortTimeout {
    /// Constructs a timeout from a non-zero duration.
    pub const fn new(value: Duration) -> Result<Self, AppPortTimeoutError> {
        if value.is_zero() {
            return Err(AppPortTimeoutError::Zero);
        }

        Ok(Self(value))
    }

    /// Returns the timeout duration.
    pub const fn duration(self) -> Duration {
        self.0
    }
}

/// Port timeout validation error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AppPortTimeoutError {
    /// Timeout values must be non-zero.
    #[error("app port timeout must be greater than zero")]
    Zero,
}

#[cfg(test)]
#[path = "timeout_tests.rs"]
mod tests;
