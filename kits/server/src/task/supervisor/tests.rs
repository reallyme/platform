// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::{oneshot, watch};

use super::{
    BackgroundTaskSet, DEFAULT_BACKGROUND_TASK_CAPACITY, MAX_BACKGROUND_TASK_CAPACITY,
    TaskSetCapacity,
};
use crate::shutdown::{ShutdownError, ShutdownReason};
use crate::startup::TaskName;
use crate::task::{
    ShutdownTimeout, TaskExecutionError, TaskExecutionErrorKind, TaskSetError,
    TaskSetValidationErrorReason,
};

struct DropFlag {
    dropped: Arc<AtomicBool>,
}

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn background_tasks_receive_shutdown_signal_and_finish_cleanly() {
    let mut tasks = BackgroundTaskSet::new();
    let (reason_sender, reason_receiver) = oneshot::channel();
    let task_name = TaskName::new("readiness-probe").expect("valid task name");

    tasks
        .spawn(task_name, move |mut shutdown| async move {
            let reason = shutdown.cancelled().await;
            let _ = reason_sender.send(reason);
        })
        .expect("task registration should succeed");

    let shutdown_timeout =
        ShutdownTimeout::new(Duration::from_secs(1)).expect("valid shutdown timeout");

    let result = tasks
        .shutdown(ShutdownReason::Sigterm, shutdown_timeout)
        .await;

    assert_eq!(result, Ok(()));
    assert_eq!(
        reason_receiver
            .await
            .expect("task should report its shutdown reason"),
        ShutdownReason::Sigterm
    );
}

#[tokio::test]
async fn background_tasks_drain_concurrently_against_shared_deadline() {
    let mut tasks = BackgroundTaskSet::new();

    tasks
        .spawn(
            TaskName::new("first-worker").expect("valid fixture task name"),
            |_shutdown| async {
                tokio::time::sleep(Duration::from_millis(40)).await;
            },
        )
        .expect("task registration should succeed");

    tasks
        .spawn(
            TaskName::new("second-worker").expect("valid fixture task name"),
            |_shutdown| async {
                tokio::time::sleep(Duration::from_millis(40)).await;
            },
        )
        .expect("task registration should succeed");

    let shutdown_timeout =
        ShutdownTimeout::new(Duration::from_millis(60)).expect("valid shutdown timeout");

    let result = tasks
        .shutdown(ShutdownReason::Sigterm, shutdown_timeout)
        .await;

    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn timed_out_tasks_are_aborted_and_reported() {
    let mut tasks = BackgroundTaskSet::new();
    let dropped = Arc::new(AtomicBool::new(false));
    let task_name = TaskName::new("stuck-worker").expect("valid task name");
    let expected_task_name = task_name.clone();
    let dropped_for_task = Arc::clone(&dropped);

    tasks
        .spawn(task_name, move |_shutdown| async move {
            let _drop_flag = DropFlag {
                dropped: dropped_for_task,
            };

            future::pending::<()>().await;
        })
        .expect("task registration should succeed");

    let shutdown_timeout =
        ShutdownTimeout::new(Duration::from_millis(20)).expect("valid shutdown timeout");

    let result = tasks
        .shutdown(ShutdownReason::Sigterm, shutdown_timeout)
        .await;

    assert_eq!(
        result,
        Err(ShutdownError::TaskShutdownTimedOut {
            task_name: expected_task_name,
            timeout: shutdown_timeout,
        })
    );
    assert!(
        dropped.load(Ordering::SeqCst),
        "timed out task should be aborted and dropped before shutdown returns"
    );
}

#[tokio::test]
async fn cannot_register_tasks_after_shutdown_begins() {
    let mut tasks = BackgroundTaskSet::new();
    let task_name = TaskName::new("late-worker").expect("valid task name");
    let expected_task_name = task_name.clone();

    assert!(tasks.begin_shutdown(ShutdownReason::CtrlC));

    let result = tasks.spawn(task_name, |_shutdown| async {});

    assert_eq!(
        result,
        Err(ShutdownError::ShutdownAlreadyRequested {
            task_name: expected_task_name,
        })
    );
}

#[test]
fn rejects_zero_task_set_capacity() {
    assert_eq!(
        TaskSetCapacity::new(0),
        Err(TaskSetError::new(
            TaskSetValidationErrorReason::MustBeGreaterThanZero
        ))
    );
}

#[test]
fn rejects_task_set_capacity_above_platform_maximum() {
    assert_eq!(
        TaskSetCapacity::new(MAX_BACKGROUND_TASK_CAPACITY.as_usize() + 1),
        Err(TaskSetError::new(
            TaskSetValidationErrorReason::MustBeLessThanOrEqualToMaximum
        ))
    );
}

#[test]
fn accepts_task_set_capacity_boundary_values() {
    assert!(TaskSetCapacity::new(1).is_ok());
    assert!(TaskSetCapacity::new(MAX_BACKGROUND_TASK_CAPACITY.as_usize()).is_ok());
}

#[test]
fn default_task_set_capacity_is_explicit() {
    let tasks = BackgroundTaskSet::new();

    assert_eq!(tasks.task_capacity(), DEFAULT_BACKGROUND_TASK_CAPACITY);
}

#[tokio::test]
async fn task_registration_respects_configured_capacity() {
    let capacity = TaskSetCapacity::new(1).expect("valid task-set capacity");
    let mut tasks = BackgroundTaskSet::with_capacity(capacity);
    let first_name = TaskName::new("first-worker").expect("valid task name");
    let second_name = TaskName::new("second-worker").expect("valid task name");
    let expected_second_name = second_name.clone();

    tasks
        .spawn(first_name, |_shutdown| async {
            future::pending::<()>().await;
        })
        .expect("first task should fit within capacity");

    let result = tasks.spawn(second_name, |_shutdown| async {});

    assert_eq!(
        result,
        Err(ShutdownError::TaskCapacityExceeded {
            task_name: expected_second_name,
            capacity,
        })
    );
}

#[tokio::test]
async fn fallible_task_failure_is_reported_during_shutdown() {
    let mut tasks = BackgroundTaskSet::new();
    let task_name = TaskName::new("dependency-watcher").expect("valid task name");
    let expected_task_name = task_name.clone();

    tasks
        .spawn_fallible(task_name, |_shutdown| async move {
            Err(TaskExecutionError::new(
                TaskExecutionErrorKind::DependencyUnavailable,
            ))
        })
        .expect("fallible task registration should succeed");

    let shutdown_timeout =
        ShutdownTimeout::new(Duration::from_secs(1)).expect("valid shutdown timeout");

    let result = tasks
        .shutdown(ShutdownReason::Sigterm, shutdown_timeout)
        .await;

    assert_eq!(
        result,
        Err(ShutdownError::TaskExitedWithError {
            task_name: expected_task_name,
            kind: TaskExecutionErrorKind::DependencyUnavailable,
        })
    );
}

#[tokio::test]
async fn finished_tasks_can_be_reaped_before_shutdown() {
    let mut tasks = BackgroundTaskSet::new();
    let task_name = TaskName::new("config-reloader").expect("valid task name");
    let expected_task_name = task_name.clone();

    tasks
        .spawn_fallible(task_name, |_shutdown| async move {
            Err(TaskExecutionError::new(
                TaskExecutionErrorKind::InvalidConfiguration,
            ))
        })
        .expect("fallible task registration should succeed");

    tokio::task::yield_now().await;

    let result = tasks.reap_finished_tasks().await;

    assert_eq!(
        result,
        Err(ShutdownError::TaskExitedWithError {
            task_name: expected_task_name,
            kind: TaskExecutionErrorKind::InvalidConfiguration,
        })
    );
    assert_eq!(tasks.tracked_task_count(), 0);
}

#[tokio::test]
async fn dropped_task_set_aborts_tracked_tasks() {
    let dropped = Arc::new(AtomicBool::new(false));
    let dropped_for_task = Arc::clone(&dropped);
    let (started_sender, started_receiver) = watch::channel(false);

    {
        let mut tasks = BackgroundTaskSet::new();
        let task_name = TaskName::new("drop-guarded-worker").expect("valid task name");

        tasks
            .spawn(task_name, move |_shutdown| async move {
                let _ = started_sender.send(true);
                let _drop_flag = DropFlag {
                    dropped: dropped_for_task,
                };

                future::pending::<()>().await;
            })
            .expect("task registration should succeed");

        let mut started_receiver = started_receiver;
        started_receiver
            .changed()
            .await
            .expect("task start signal should arrive before drop");
        assert!(
            *started_receiver.borrow(),
            "task should report that it has started before the supervisor is dropped"
        );
    }

    tokio::task::yield_now().await;

    assert!(
        dropped.load(Ordering::SeqCst),
        "dropping the task set should abort tracked tasks instead of orphaning them"
    );
}
