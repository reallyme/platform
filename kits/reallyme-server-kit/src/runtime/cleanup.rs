// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;
use std::time::Instant;

use reallyme_app_kit::AppCleanupHookDescriptor;
use tokio::time;

use crate::startup::{StartupError, TaskName};
use crate::task::{ShutdownTimeout, TaskExecutionError, TaskExecutionErrorKind};

use super::app::RuntimeAppCleanup;
use super::error::{RuntimeAppCleanupErrorReason, ServerRuntimeError};

type BoxedCleanupFuture = Pin<Box<dyn Future<Output = Result<(), TaskExecutionError>> + Send>>;
type BoxedCleanupHook = Box<dyn FnOnce() -> BoxedCleanupFuture + Send + 'static>;

/// App-provided cleanup hook executed during runtime shutdown.
///
/// Cleanup hooks are for app-owned resources only: client pools, final metrics,
/// buffered handles, or similar app-local state. They run outside the request
/// hot path and are bounded by the runtime cleanup timeout so a broken app
/// cannot keep the server process alive forever.
pub struct RuntimeCleanupHook {
    name: TaskName,
    cleanup: BoxedCleanupHook,
}

impl RuntimeCleanupHook {
    /// Creates a named cleanup hook.
    pub fn new<F, Fut>(name: TaskName, cleanup: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), TaskExecutionError>> + Send + 'static,
    {
        Self {
            name,
            cleanup: Box::new(move || Box::pin(cleanup())),
        }
    }

    /// Builds a runtime cleanup hook from a host-neutral app descriptor.
    ///
    /// Anchors the runtime cleanup hook to the name advertised in the
    /// [`reallyme_app_kit::AppDescriptor`], so a cleanup hook declared by the
    /// app contract cannot silently disappear from the runtime.
    pub fn from_app_descriptor<F, Fut>(
        descriptor: &AppCleanupHookDescriptor,
        cleanup: F,
    ) -> Result<Self, StartupError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), TaskExecutionError>> + Send + 'static,
    {
        let name = TaskName::new(descriptor.name().as_str())?;
        Ok(Self::new(name, cleanup))
    }

    pub(crate) fn name(&self) -> TaskName {
        self.name.clone()
    }

    pub(crate) async fn run(self, timeout: ShutdownTimeout) -> RuntimeCleanupResult {
        match time::timeout(timeout.as_duration(), (self.cleanup)()).await {
            Ok(Ok(())) => RuntimeCleanupResult::Completed,
            Ok(Err(error)) => RuntimeCleanupResult::Failed { kind: error.kind() },
            Err(_) => RuntimeCleanupResult::TimedOut { timeout },
        }
    }
}

/// Result of one bounded app cleanup hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeCleanupResult {
    /// Cleanup finished successfully.
    Completed,
    /// Cleanup returned a typed operational failure.
    Failed {
        /// Low-cardinality failure kind.
        kind: TaskExecutionErrorKind,
    },
    /// Cleanup exceeded its configured timeout.
    TimedOut {
        /// Timeout enforced by the runtime.
        timeout: ShutdownTimeout,
    },
}

pub(crate) async fn run_cleanup_hooks(
    cleanup_hooks: Vec<RuntimeAppCleanup>,
    cleanup_timeout: ShutdownTimeout,
    server_name: &crate::startup::ServerName,
) -> Result<(), ServerRuntimeError> {
    let mut first_error = None;
    let cleanup_deadline = Instant::now() + cleanup_timeout.as_duration();

    for cleanup in cleanup_hooks.into_iter().rev() {
        let app_name = cleanup.app_name;
        let hook_name = cleanup.hook.name();

        let now = Instant::now();
        if now >= cleanup_deadline {
            first_error.get_or_insert(ServerRuntimeError::AppCleanup {
                hook_name: hook_name.clone(),
                reason: RuntimeAppCleanupErrorReason::TimedOut,
            });
            record_cleanup_failure(server_name, &app_name, &hook_name);
            break;
        }

        let mut remaining_timeout = cleanup_deadline.duration_since(now);
        if remaining_timeout.is_zero() {
            remaining_timeout = std::time::Duration::from_nanos(1);
        }
        let remaining_timeout = match ShutdownTimeout::new(remaining_timeout) {
            Ok(timeout) => timeout,
            Err(_) => {
                first_error.get_or_insert(ServerRuntimeError::AppCleanup {
                    hook_name: hook_name.clone(),
                    reason: RuntimeAppCleanupErrorReason::TimedOut,
                });
                record_cleanup_failure(server_name, &app_name, &hook_name);
                break;
            }
        };

        crate::observability::log_runtime_app_cleanup_started(server_name, &app_name, &hook_name);

        match cleanup.hook.run(remaining_timeout).await {
            RuntimeCleanupResult::Completed => {
                crate::observability::log_runtime_app_cleanup_completed(
                    server_name,
                    &app_name,
                    &hook_name,
                );
            }
            RuntimeCleanupResult::Failed { kind } => {
                crate::observability::record_runtime_app_cleanup_failure(
                    crate::observability::RuntimeAppFailureOutcome::Failed,
                );
                crate::observability::log_runtime_app_cleanup_failed(
                    server_name,
                    &app_name,
                    &hook_name,
                    kind,
                );
                first_error.get_or_insert(ServerRuntimeError::AppCleanup {
                    hook_name,
                    reason: RuntimeAppCleanupErrorReason::Failed { kind },
                });
            }
            RuntimeCleanupResult::TimedOut { timeout: _ } => {
                record_cleanup_failure(server_name, &app_name, &hook_name);
                first_error.get_or_insert(ServerRuntimeError::AppCleanup {
                    hook_name,
                    reason: RuntimeAppCleanupErrorReason::TimedOut,
                });
            }
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn record_cleanup_failure(
    server_name: &crate::startup::ServerName,
    app_name: &super::app::AppName,
    hook_name: &TaskName,
) {
    crate::observability::record_runtime_app_cleanup_failure(
        crate::observability::RuntimeAppFailureOutcome::TimedOut,
    );
    crate::observability::log_runtime_app_cleanup_failed(
        server_name,
        app_name,
        hook_name,
        TaskExecutionErrorKind::Internal,
    );
}

#[cfg(test)]
mod tests {
    use std::future;
    use std::time::Duration;

    use super::{RuntimeCleanupHook, RuntimeCleanupResult};
    use crate::startup::TaskName;
    use crate::task::{ShutdownTimeout, TaskExecutionError, TaskExecutionErrorKind};

    #[tokio::test]
    async fn cleanup_hook_reports_success() {
        let hook = RuntimeCleanupHook::new(
            TaskName::new("cleanup-success").expect("valid fixture task name"),
            || async { Ok::<(), TaskExecutionError>(()) },
        );
        let timeout = ShutdownTimeout::new(Duration::from_secs(1)).expect("valid fixture timeout");

        assert_eq!(hook.run(timeout).await, RuntimeCleanupResult::Completed);
    }

    #[tokio::test]
    async fn cleanup_hook_reports_typed_failure() {
        let hook = RuntimeCleanupHook::new(
            TaskName::new("cleanup-failure").expect("valid fixture task name"),
            || async {
                Err(TaskExecutionError::new(
                    TaskExecutionErrorKind::DependencyUnavailable,
                ))
            },
        );
        let timeout = ShutdownTimeout::new(Duration::from_secs(1)).expect("valid fixture timeout");

        assert_eq!(
            hook.run(timeout).await,
            RuntimeCleanupResult::Failed {
                kind: TaskExecutionErrorKind::DependencyUnavailable,
            }
        );
    }

    #[tokio::test]
    async fn cleanup_hook_timeout_is_bounded() {
        let hook = RuntimeCleanupHook::new(
            TaskName::new("cleanup-timeout").expect("valid fixture task name"),
            || async {
                future::pending::<()>().await;
                Ok::<(), TaskExecutionError>(())
            },
        );
        let timeout =
            ShutdownTimeout::new(Duration::from_millis(10)).expect("valid fixture timeout");

        assert_eq!(
            hook.run(timeout).await,
            RuntimeCleanupResult::TimedOut { timeout }
        );
    }
}
