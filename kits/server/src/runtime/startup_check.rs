// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use reallyme_app_kit::AppStartupCheckDescriptor;
use thiserror::Error;

use crate::startup::{StartupError, TaskName};
use crate::task::{TaskExecutionError, TaskExecutionErrorKind};

type BoxedStartupCheckFuture = Pin<Box<dyn Future<Output = Result<(), TaskExecutionError>> + Send>>;
type BoxedStartupCheck = Box<dyn FnOnce() -> BoxedStartupCheckFuture + Send + 'static>;

const DEFAULT_STARTUP_CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_STARTUP_CHECK_TIMEOUT: Duration = Duration::from_secs(300);

/// Bounded deadline for one app startup check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupCheckTimeout(Duration);

/// Validation reason for an app startup-check deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupCheckTimeoutErrorReason {
    /// The deadline is zero.
    MustBeGreaterThanZero,
    /// The deadline exceeds the allowed operational bound.
    ExceedsMaximum,
}

/// Typed startup-check deadline validation failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("invalid startup check timeout")]
pub struct StartupCheckTimeoutError {
    reason: StartupCheckTimeoutErrorReason,
}

impl StartupCheckTimeoutError {
    /// Returns the stable validation reason.
    pub const fn reason(self) -> StartupCheckTimeoutErrorReason {
        self.reason
    }
}

impl StartupCheckTimeout {
    /// Validates a deadline for one startup check.
    pub fn new(value: Duration) -> Result<Self, StartupCheckTimeoutError> {
        if value.is_zero() {
            return Err(StartupCheckTimeoutError {
                reason: StartupCheckTimeoutErrorReason::MustBeGreaterThanZero,
            });
        }
        if value > MAX_STARTUP_CHECK_TIMEOUT {
            return Err(StartupCheckTimeoutError {
                reason: StartupCheckTimeoutErrorReason::ExceedsMaximum,
            });
        }
        Ok(Self(value))
    }
}

/// Service-provided startup readiness gate executed before runtime readiness.
///
/// Startup checks let app crates prove critical dependencies or required
/// background initialization before `ServerRuntime` marks the process ready.
/// They are intentionally outside the request hot path, so boxed futures are
/// acceptable here to allow heterogeneous app-owned checks.
pub struct RuntimeStartupCheck {
    name: TaskName,
    check: BoxedStartupCheck,
    timeout: StartupCheckTimeout,
}

impl RuntimeStartupCheck {
    /// Creates a named startup check.
    pub fn new<F, Fut>(name: TaskName, check: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), TaskExecutionError>> + Send + 'static,
    {
        Self {
            name,
            check: Box::new(move || Box::pin(check())),
            timeout: StartupCheckTimeout(DEFAULT_STARTUP_CHECK_TIMEOUT),
        }
    }

    /// Builds a runtime startup check from a host-neutral app descriptor.
    ///
    /// The runtime name is derived from the descriptor so app authors cannot
    /// declare a startup check in the [`reallyme_app_kit::AppDescriptor`] and
    /// then accidentally register the runtime hook under a divergent name.
    pub fn from_app_descriptor<F, Fut>(
        descriptor: &AppStartupCheckDescriptor,
        check: F,
    ) -> Result<Self, StartupError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), TaskExecutionError>> + Send + 'static,
    {
        let name = TaskName::new(descriptor.name().as_str())?;
        Ok(Self::new(name, check))
    }

    pub(crate) fn name(&self) -> TaskName {
        self.name.clone()
    }

    /// Sets the maximum time allowed for this check to complete.
    pub fn with_timeout(mut self, timeout: StartupCheckTimeout) -> Self {
        self.timeout = timeout;
        self
    }

    pub(crate) async fn run(self) -> Result<(), TaskExecutionErrorKind> {
        tokio::time::timeout(self.timeout.0, (self.check)())
            .await
            .map_err(|_| TaskExecutionErrorKind::TimedOut)?
            .map_err(TaskExecutionError::kind)
    }
}

#[cfg(test)]
#[path = "startup_check_tests.rs"]
mod tests;
