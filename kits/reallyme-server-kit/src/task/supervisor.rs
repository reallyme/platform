// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::mem;

use tokio::task::JoinSet;
use tokio::time::{self, Instant};

use super::error::{TaskExecutionError, TaskSetError, TaskSetValidationErrorReason};
use super::shutdown::{ShutdownController, ShutdownTimeout, ShutdownToken};
use super::spawn::{
    ManagedBackgroundTask, spawn_managed_background_task, spawn_managed_fallible_background_task,
};
#[cfg(feature = "metrics")]
use crate::observability::{RuntimeQueueLabel, record_runtime_queue_saturation};
use crate::shutdown::{ShutdownError, ShutdownReason, TaskJoinFailureReason};
use crate::startup::TaskName;

const DEFAULT_BACKGROUND_TASK_CAPACITY_VALUE: usize = 1024;
const MAX_BACKGROUND_TASK_CAPACITY_VALUE: usize = 16_384;

/// Default maximum number of tracked background tasks in a task set.
pub const DEFAULT_BACKGROUND_TASK_CAPACITY: TaskSetCapacity =
    TaskSetCapacity(DEFAULT_BACKGROUND_TASK_CAPACITY_VALUE);
/// Platform maximum number of tracked background tasks in a task set.
///
/// The task set owns one join handle per task. Keeping a reviewed maximum
/// prevents accidental unbounded lifecycle tracking in server processes.
pub const MAX_BACKGROUND_TASK_CAPACITY: TaskSetCapacity =
    TaskSetCapacity(MAX_BACKGROUND_TASK_CAPACITY_VALUE);

/// Validated maximum number of tracked background tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskSetCapacity(usize);

impl TaskSetCapacity {
    /// Constructs a validated background-task set capacity.
    pub fn new(value: usize) -> Result<Self, TaskSetError> {
        if value == 0 {
            return Err(TaskSetError::new(
                TaskSetValidationErrorReason::MustBeGreaterThanZero,
            ));
        }

        if value > MAX_BACKGROUND_TASK_CAPACITY_VALUE {
            return Err(TaskSetError::new(
                TaskSetValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            ));
        }

        Ok(Self(value))
    }

    /// Returns the validated capacity as a plain `usize`.
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

/// Owns long-lived background tasks and coordinates their shutdown.
///
/// The set deliberately owns every spawned [`tokio::task::JoinHandle`] so
/// callers cannot accidentally drop a handle and detach a task during graceful
/// shutdown. When shutdown begins, tasks receive a shared [`ShutdownToken`] and
/// are then drained with an explicit timeout before any remaining work is
/// aborted.
///
/// Apps should prefer this type over ad hoc `tokio::spawn` calls for
/// long-lived background work so lifecycle ownership, cancellation, and
/// shutdown deadlines remain centralized and reviewable.
pub struct BackgroundTaskSet {
    shutdown: ShutdownController,
    tasks: Vec<ManagedBackgroundTask>,
    capacity: TaskSetCapacity,
}

impl Default for BackgroundTaskSet {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BackgroundTaskSet {
    fn drop(&mut self) {
        let _ = self.begin_shutdown(ShutdownReason::Drop);

        for task in &self.tasks {
            task.handle.abort();
        }
    }
}

impl BackgroundTaskSet {
    /// Creates an empty background task set.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_BACKGROUND_TASK_CAPACITY)
    }

    /// Creates an empty background task set with an explicit capacity.
    pub fn with_capacity(capacity: TaskSetCapacity) -> Self {
        Self {
            shutdown: ShutdownController::new(),
            tasks: Vec::new(),
            capacity,
        }
    }

    /// Returns a token subscribed to this task set's shutdown controller.
    pub fn shutdown_token(&self) -> ShutdownToken {
        self.shutdown.token()
    }

    /// Returns whether shutdown has already begun.
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown.is_shutdown_requested()
    }

    /// Returns the number of tracked tasks that have not yet been removed.
    pub fn tracked_task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Returns the maximum number of tracked tasks this set accepts.
    pub const fn task_capacity(&self) -> TaskSetCapacity {
        self.capacity
    }

    /// Returns the active shutdown reason, if one has been set.
    pub fn shutdown_reason(&self) -> Option<ShutdownReason> {
        self.shutdown.shutdown_reason()
    }

    /// Begins coordinated shutdown.
    ///
    /// Returns `true` only on the first successful transition.
    pub fn begin_shutdown(&self, reason: ShutdownReason) -> bool {
        self.shutdown.begin_shutdown(reason)
    }

    /// Spawns a named background task that receives a clone of the shared
    /// shutdown token.
    ///
    /// Callers should register all long-lived tasks before the service begins
    /// shutting down. Registering after shutdown begins is rejected so the task
    /// set cannot grow while it is draining.
    pub fn spawn<F, Fut>(&mut self, task_name: TaskName, task: F) -> Result<(), ShutdownError>
    where
        F: FnOnce(ShutdownToken) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        if self.is_shutdown_requested() {
            return Err(ShutdownError::ShutdownAlreadyRequested { task_name });
        }

        if self.tasks.len() >= self.capacity.as_usize() {
            #[cfg(feature = "metrics")]
            record_runtime_queue_saturation(RuntimeQueueLabel::BackgroundTaskSet);
            return Err(ShutdownError::TaskCapacityExceeded {
                task_name,
                capacity: self.capacity,
            });
        }

        self.tasks.push(spawn_managed_background_task(
            task_name,
            task,
            self.shutdown.token(),
        ));

        Ok(())
    }

    /// Spawns a named background task that may return a typed operational
    /// failure.
    ///
    /// This is the preferred helper for long-lived loops that need to surface
    /// dependency loss or other infrastructure-safe failure kinds without
    /// leaking rich internal error detail into logs or shutdown surfaces.
    pub fn spawn_fallible<F, Fut>(
        &mut self,
        task_name: TaskName,
        task: F,
    ) -> Result<(), ShutdownError>
    where
        F: FnOnce(ShutdownToken) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), TaskExecutionError>> + Send + 'static,
    {
        if self.is_shutdown_requested() {
            return Err(ShutdownError::ShutdownAlreadyRequested { task_name });
        }

        if self.tasks.len() >= self.capacity.as_usize() {
            #[cfg(feature = "metrics")]
            record_runtime_queue_saturation(RuntimeQueueLabel::BackgroundTaskSet);
            return Err(ShutdownError::TaskCapacityExceeded {
                task_name,
                capacity: self.capacity,
            });
        }

        self.tasks.push(spawn_managed_fallible_background_task(
            task_name,
            task,
            self.shutdown.token(),
        ));

        Ok(())
    }

    /// Signals shutdown and drains all tracked tasks with the configured
    /// timeout.
    pub async fn shutdown(
        &mut self,
        reason: ShutdownReason,
        timeout: ShutdownTimeout,
    ) -> Result<(), ShutdownError> {
        let _ = self.begin_shutdown(reason);
        self.wait_for_tasks(timeout).await
    }

    /// Waits for all tracked tasks to finish within one shared timeout.
    ///
    /// Callers are expected to request shutdown before invoking this method so
    /// tasks have an opportunity to observe their cancellation token and exit
    /// cleanly. The timeout is a global drain deadline for the task set, not a
    /// per-task allowance, so shutdown latency remains bounded even when
    /// multiple tasks are stuck.
    pub async fn wait_for_tasks(&mut self, timeout: ShutdownTimeout) -> Result<(), ShutdownError> {
        let mut first_error: Option<ShutdownError> = None;
        let shutdown_deadline = Instant::now() + timeout.as_duration();

        let mut tracked_tasks = mem::take(&mut self.tasks);
        let mut pending_names: Vec<TaskName> = Vec::with_capacity(tracked_tasks.len());
        let mut abort_handles: Vec<(TaskName, tokio::task::AbortHandle)> =
            Vec::with_capacity(tracked_tasks.len());
        let mut join_set = JoinSet::new();

        while let Some(task) = tracked_tasks.pop() {
            let task_name = task.name;
            pending_names.push(task_name.clone());
            abort_handles.push((task_name.clone(), task.handle.abort_handle()));

            join_set.spawn(async move {
                let completion = task.handle.await;
                (task_name, completion)
            });
        }

        while let Ok(Some(task_result)) =
            time::timeout_at(shutdown_deadline, join_set.join_next()).await
        {
            match task_result {
                Ok((task_name, join_result)) => {
                    if let Some(pos) = pending_names.iter().position(|name| name == &task_name) {
                        pending_names.remove(pos);
                    }

                    if let Some(error) = map_task_completion(task_name, join_result) {
                        first_error.get_or_insert(error);
                    }
                }
                Err(_) => {
                    if let Some(task_name) = pending_names.pop() {
                        first_error.get_or_insert(ShutdownError::TaskJoinFailed {
                            task_name,
                            reason: TaskJoinFailureReason::Unknown,
                        });
                    };
                }
            }
        }

        if !join_set.is_empty() {
            let timed_out_task_name = abort_handles
                .first()
                .map(|(task_name, _)| task_name.clone());
            for (_, abort_handle) in abort_handles {
                abort_handle.abort();
            }

            while let Some(task_result) = join_set.join_next().await {
                if let Ok((task_name, join_result)) = task_result {
                    if let Some(pos) = pending_names.iter().position(|name| name == &task_name) {
                        pending_names.remove(pos);
                    }
                    let task_error = map_task_completion(task_name, join_result);
                    if let Some(ShutdownError::TaskJoinFailed {
                        reason: TaskJoinFailureReason::Cancelled,
                        ..
                    }) = task_error
                    {
                        continue;
                    }
                    if let Some(error) = task_error {
                        first_error.get_or_insert(error);
                    }
                }
            }

            if let Some(task_name) = timed_out_task_name {
                first_error
                    .get_or_insert(ShutdownError::TaskShutdownTimedOut { task_name, timeout });
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Reaps already-finished tasks without waiting for active tasks.
    ///
    /// Server runtimes may call this periodically in supervisory loops to
    /// detect task failure or panic before global shutdown begins.
    pub async fn reap_finished_tasks(&mut self) -> Result<(), ShutdownError> {
        let mut first_error: Option<ShutdownError> = None;

        let mut index = 0usize;

        while index < self.tasks.len() {
            if self.tasks[index].handle.is_finished() {
                let task = self.tasks.swap_remove(index);

                if let Some(error) = map_task_completion(task.name, task.handle.await) {
                    first_error.get_or_insert(error);
                }
            } else {
                index += 1;
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn map_task_completion(
    task_name: TaskName,
    completion: Result<Result<(), TaskExecutionError>, tokio::task::JoinError>,
) -> Option<ShutdownError> {
    match completion {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(ShutdownError::TaskExitedWithError {
            task_name,
            kind: error.kind(),
        }),
        Err(join_error) => Some(ShutdownError::TaskJoinFailed {
            task_name,
            reason: map_join_failure_reason(&join_error),
        }),
    }
}

fn map_join_failure_reason(join_error: &tokio::task::JoinError) -> TaskJoinFailureReason {
    if join_error.is_cancelled() {
        return TaskJoinFailureReason::Cancelled;
    }

    if join_error.is_panic() {
        return TaskJoinFailureReason::Panicked;
    }

    TaskJoinFailureReason::Unknown
}

#[cfg(test)]
mod tests {
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
}
