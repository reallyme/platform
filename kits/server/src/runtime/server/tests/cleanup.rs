// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::fixtures::cleanup_entry;
use crate::runtime::app::RuntimeAppCleanup;
use crate::runtime::{
    AppName, RuntimeAppCleanupErrorReason, RuntimeCleanupHook, ServerRuntimeError,
};
use crate::startup::{ServerName, TaskName};
use crate::task::{ShutdownTimeout, TaskExecutionError};

#[tokio::test]
async fn app_cleanup_hook_runs() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let cleanup_hooks = vec![cleanup_entry("api", "api-cleanup", "api", &observed)];
    let timeout =
        ShutdownTimeout::new(Duration::from_secs(1)).expect("valid fixture cleanup timeout");
    let server_name = ServerName::new("cleanup-fixture").expect("valid fixture server name");

    let result = super::super::run_cleanup_hooks(cleanup_hooks, timeout, &server_name).await;

    assert!(result.is_ok());
    assert_eq!(
        observed
            .lock()
            .expect("test mutex should not be poisoned")
            .as_slice(),
        ["api"]
    );
}

#[tokio::test]
async fn app_cleanup_runs_in_reverse_startup_order() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let cleanup_hooks = vec![
        cleanup_entry("dependency", "dependency-cleanup", "dependency", &observed),
        cleanup_entry("dependent", "dependent-cleanup", "dependent", &observed),
    ];
    let timeout =
        ShutdownTimeout::new(Duration::from_secs(1)).expect("valid fixture cleanup timeout");
    let server_name = ServerName::new("cleanup-order-fixture").expect("valid fixture server name");

    let result = super::super::run_cleanup_hooks(cleanup_hooks, timeout, &server_name).await;

    assert!(result.is_ok());
    assert_eq!(
        observed
            .lock()
            .expect("test mutex should not be poisoned")
            .as_slice(),
        ["dependent", "dependency"]
    );
}

#[tokio::test]
async fn app_cleanup_timeout_is_reported() {
    let hook_name = TaskName::new("stuck-cleanup").expect("valid fixture task name");
    let cleanup_hooks = vec![RuntimeAppCleanup {
        app_name: AppName::new("api").expect("valid fixture app name"),
        hook: RuntimeCleanupHook::new(hook_name.clone(), || async {
            std::future::pending::<()>().await;
            Ok::<(), TaskExecutionError>(())
        }),
    }];
    let timeout = ShutdownTimeout::new(Duration::from_millis(10)).expect("valid fixture timeout");
    let server_name =
        ServerName::new("cleanup-timeout-fixture").expect("valid fixture server name");

    let result = super::super::run_cleanup_hooks(cleanup_hooks, timeout, &server_name).await;

    assert!(matches!(
        result,
        Err(ServerRuntimeError::AppCleanup {
            hook_name: actual_hook_name,
            reason: RuntimeAppCleanupErrorReason::TimedOut,
        }) if actual_hook_name == hook_name
    ));
}

#[tokio::test]
async fn app_cleanup_failure_is_reported() {
    let hook_name = TaskName::new("failing-cleanup").expect("valid fixture task name");
    let cleanup_hooks = vec![RuntimeAppCleanup {
        app_name: AppName::new("api").expect("valid fixture app name"),
        hook: RuntimeCleanupHook::new(hook_name.clone(), || async {
            Err(TaskExecutionError::new(
                crate::task::TaskExecutionErrorKind::DependencyUnavailable,
            ))
        }),
    }];
    let timeout = ShutdownTimeout::new(Duration::from_secs(1)).expect("valid fixture timeout");
    let server_name =
        ServerName::new("cleanup-failure-fixture").expect("valid fixture server name");

    let result = super::super::run_cleanup_hooks(cleanup_hooks, timeout, &server_name).await;

    assert!(matches!(
        result,
        Err(ServerRuntimeError::AppCleanup {
            hook_name: actual_hook_name,
            reason: RuntimeAppCleanupErrorReason::Failed {
                kind: crate::task::TaskExecutionErrorKind::DependencyUnavailable,
            },
        }) if actual_hook_name == hook_name
    ));
}

#[tokio::test]
async fn app_cleanup_hooks_share_global_timeout_budget() {
    let timed_out_hook = Arc::new(AtomicUsize::new(0));
    let skipped_hook = Arc::new(AtomicUsize::new(0));
    let second = Arc::clone(&timed_out_hook);
    let first = Arc::clone(&skipped_hook);

    let hook_first = RuntimeAppCleanup {
        app_name: AppName::new("api").expect("valid fixture app name"),
        hook: RuntimeCleanupHook::new(
            TaskName::new("skipped-hook").expect("valid fixture task name"),
            move || {
                first.fetch_add(1, Ordering::SeqCst);
                async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok::<(), TaskExecutionError>(())
                }
            },
        ),
    };
    let hook_second = RuntimeAppCleanup {
        app_name: AppName::new("api").expect("valid fixture app name"),
        hook: RuntimeCleanupHook::new(
            TaskName::new("timed-out-hook").expect("valid fixture task name"),
            move || {
                second.fetch_add(1, Ordering::SeqCst);
                async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok::<(), TaskExecutionError>(())
                }
            },
        ),
    };
    let cleanup_hooks = vec![hook_first, hook_second];
    let timeout = ShutdownTimeout::new(Duration::from_millis(30)).expect("valid fixture timeout");
    let server_name =
        ServerName::new("cleanup-global-timeout-fixture").expect("valid fixture server name");

    let result = super::super::run_cleanup_hooks(cleanup_hooks, timeout, &server_name).await;

    assert!(result.is_err());
    assert_eq!(skipped_hook.load(Ordering::SeqCst), 0);
    assert_eq!(timed_out_hook.load(Ordering::SeqCst), 1);
    let timed_out_hook_name = TaskName::new("timed-out-hook").expect("valid fixture task name");
    assert!(matches!(
        result,
        Err(ServerRuntimeError::AppCleanup {
            hook_name: actual_hook_name,
            reason: RuntimeAppCleanupErrorReason::TimedOut,
        }) if actual_hook_name == timed_out_hook_name
    ));
}
