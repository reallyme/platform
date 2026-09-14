// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;

use reallyme_app_kit::AppStartupCheckDescriptor;

use crate::startup::{StartupError, TaskName};
use crate::task::{TaskExecutionError, TaskExecutionErrorKind};

type BoxedStartupCheckFuture = Pin<Box<dyn Future<Output = Result<(), TaskExecutionError>> + Send>>;
type BoxedStartupCheck = Box<dyn FnOnce() -> BoxedStartupCheckFuture + Send + 'static>;

/// Service-provided startup readiness gate executed before runtime readiness.
///
/// Startup checks let app crates prove critical dependencies or required
/// background initialization before `ServerRuntime` marks the process ready.
/// They are intentionally outside the request hot path, so boxed futures are
/// acceptable here to allow heterogeneous app-owned checks.
pub struct RuntimeStartupCheck {
    name: TaskName,
    check: BoxedStartupCheck,
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

    pub(crate) async fn run(self) -> Result<(), TaskExecutionErrorKind> {
        (self.check)().await.map_err(TaskExecutionError::kind)
    }
}

#[cfg(test)]
#[path = "startup_check_tests.rs"]
mod tests;
