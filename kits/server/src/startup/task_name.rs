// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::StartupError;

/// Validated identifier for long-lived background tasks.
///
/// Task names flow into logs, traces, and shutdown diagnostics, so they use the
/// same conservative character set as server names. Keeping the surface narrow
/// avoids surprising values propagating into operational tooling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskName(String);

impl TaskName {
    /// Constructs a validated background task name.
    pub fn new(value: impl Into<String>) -> Result<Self, StartupError> {
        let value = value.into();

        if value.is_empty() {
            return Err(StartupError::EmptyTaskName);
        }

        if value.len() > 63 {
            return Err(StartupError::TaskNameTooLong);
        }

        if value.starts_with('-') || value.ends_with('-') {
            return Err(StartupError::InvalidTaskNameBoundary);
        }

        if value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Ok(Self(value));
        }

        Err(StartupError::InvalidTaskName)
    }

    /// Returns the validated task name as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
