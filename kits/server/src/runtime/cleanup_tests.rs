// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
    let timeout = ShutdownTimeout::new(Duration::from_millis(10)).expect("valid fixture timeout");

    assert_eq!(
        hook.run(timeout).await,
        RuntimeCleanupResult::TimedOut { timeout }
    );
}
