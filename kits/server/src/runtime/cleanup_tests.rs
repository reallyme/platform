// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::{RuntimeCleanupHook, RuntimeCleanupResult, run_cleanup_hooks};
use crate::runtime::app::{AppName, RuntimeAppCleanup};
use crate::runtime::{RuntimeAppCleanupErrorReason, ServerRuntimeError};
use crate::startup::{ServerName, TaskName};
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

#[tokio::test]
async fn panicking_cleanup_does_not_skip_remaining_hooks() {
    let ran = Arc::new(AtomicBool::new(false));
    let later = Arc::clone(&ran);
    let app_name = AppName::new("cleanup-app").expect("valid app name");
    let hooks = vec![
        RuntimeAppCleanup {
            app_name: app_name.clone(),
            hook: RuntimeCleanupHook::new(
                TaskName::new("later-cleanup").expect("valid task name"),
                move || async move {
                    later.store(true, Ordering::SeqCst);
                    Ok::<(), TaskExecutionError>(())
                },
            ),
        },
        RuntimeAppCleanup {
            app_name,
            hook: RuntimeCleanupHook::new(
                TaskName::new("panicking-cleanup").expect("valid task name"),
                || async {
                    panic!("fixture panic");
                    #[allow(unreachable_code)]
                    Ok::<(), TaskExecutionError>(())
                },
            ),
        },
    ];
    let result = run_cleanup_hooks(
        hooks,
        ShutdownTimeout::new(Duration::from_secs(1)).expect("valid timeout"),
        &ServerName::new("cleanup-server").expect("valid server name"),
    )
    .await;
    assert!(ran.load(Ordering::SeqCst));
    assert!(matches!(
        result,
        Err(ServerRuntimeError::AppCleanup {
            reason: RuntimeAppCleanupErrorReason::Panicked,
            ..
        })
    ));
}
