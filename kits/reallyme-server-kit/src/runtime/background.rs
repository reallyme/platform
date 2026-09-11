// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;

use reallyme_app_kit::AppBackgroundTaskDescriptor;

use crate::startup::{StartupError, TaskName};
use crate::task::{ShutdownToken, TaskExecutionError};

type BoxedRuntimeTaskFuture = Pin<Box<dyn Future<Output = Result<(), TaskExecutionError>> + Send>>;
type BoxedRuntimeTaskFactory =
    Box<dyn FnOnce(ShutdownToken) -> BoxedRuntimeTaskFuture + Send + 'static>;

/// Service-specific background task registration for [`crate::runtime::ServerRuntime`].
///
/// The runtime owns spawning, shutdown signaling, join-handle tracking, and
/// timeout handling. Services own only the task name and cancellation-safe task
/// body.
///
/// Background tasks are not startup readiness gates. If a service must prove a
/// dependency is available or complete initialization before receiving traffic,
/// register a [`crate::runtime::RuntimeStartupCheck`] instead. This keeps
/// long-running support loops separate from fail-closed startup validation.
pub struct RuntimeBackgroundTask {
    task_name: TaskName,
    task: BoxedRuntimeTaskFactory,
}

impl RuntimeBackgroundTask {
    /// Creates a boxed runtime background task registration.
    pub fn new<F, Fut>(task_name: TaskName, task: F) -> Self
    where
        F: FnOnce(ShutdownToken) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), TaskExecutionError>> + Send + 'static,
    {
        Self {
            task_name,
            task: Box::new(move |shutdown| Box::pin(task(shutdown))),
        }
    }

    /// Builds a runtime background task from a host-neutral app descriptor.
    ///
    /// Keeps the descriptor's declared name in sync with the runtime task name
    /// so the [`reallyme_app_kit::AppDescriptor`] cannot list a task that never
    /// gets registered or, worse, gets registered under a divergent name.
    pub fn from_app_descriptor<F, Fut>(
        descriptor: &AppBackgroundTaskDescriptor,
        task: F,
    ) -> Result<Self, StartupError>
    where
        F: FnOnce(ShutdownToken) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), TaskExecutionError>> + Send + 'static,
    {
        let task_name = TaskName::new(descriptor.name().as_str())?;
        Ok(Self::new(task_name, task))
    }

    pub(crate) fn task_name(&self) -> TaskName {
        self.task_name.clone()
    }

    pub(crate) fn into_task(self) -> BoxedRuntimeTaskFactory {
        self.task
    }
}
