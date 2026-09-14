// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
mod tests;
