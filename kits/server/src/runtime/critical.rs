// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use reallyme_app_kit::AppBackgroundTaskDescriptor;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time;

use crate::observability::{
    log_runtime_critical_task_failed, log_runtime_critical_task_ready,
    log_runtime_critical_task_started,
};
use crate::shutdown::ShutdownError;
use crate::startup::{ServerName, StartupError, TaskName};
use crate::task::{
    BackgroundTaskSet, MAX_BACKGROUND_TASK_CAPACITY, ShutdownToken, TaskExecutionError,
    TaskExecutionErrorKind,
};

const CRITICAL_TASK_EVENTS_PER_TASK: usize = 2;
const MAX_CRITICAL_TASK_READINESS_TIMEOUT: Duration = Duration::from_secs(3_600);

type BoxedCriticalTaskFuture = Pin<Box<dyn Future<Output = Result<(), TaskExecutionError>> + Send>>;
type BoxedCriticalTaskFactory = Box<
    dyn FnOnce(ShutdownToken, CriticalTaskReadySignal) -> BoxedCriticalTaskFuture + Send + 'static,
>;

/// Validation reasons for a critical task readiness deadline.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriticalTaskReadinessTimeoutErrorReason {
    /// The timeout must allow at least one scheduler tick.
    MustBeGreaterThanZero,
    /// Startup must remain bounded to the reviewed platform maximum.
    ExceedsMaximum,
}

/// Typed validation error for a critical task readiness deadline.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("invalid critical task readiness timeout")]
pub struct CriticalTaskReadinessTimeoutError {
    reason: CriticalTaskReadinessTimeoutErrorReason,
}

impl CriticalTaskReadinessTimeoutError {
    /// Returns the low-cardinality validation reason.
    pub const fn reason(self) -> CriticalTaskReadinessTimeoutErrorReason {
        self.reason
    }
}

/// Validated deadline for a critical task to release the readiness barrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CriticalTaskReadinessTimeout(Duration);

impl CriticalTaskReadinessTimeout {
    /// Creates a non-zero readiness timeout within the reviewed one-hour bound.
    pub fn new(value: Duration) -> Result<Self, CriticalTaskReadinessTimeoutError> {
        if value.is_zero() {
            return Err(CriticalTaskReadinessTimeoutError {
                reason: CriticalTaskReadinessTimeoutErrorReason::MustBeGreaterThanZero,
            });
        }
        if value > MAX_CRITICAL_TASK_READINESS_TIMEOUT {
            return Err(CriticalTaskReadinessTimeoutError {
                reason: CriticalTaskReadinessTimeoutErrorReason::ExceedsMaximum,
            });
        }

        Ok(Self(value))
    }

    /// Returns the validated timeout as a standard duration.
    pub const fn as_duration(self) -> Duration {
        self.0
    }
}

/// Stage in which a critical runtime task caused process termination.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCriticalTaskFailureStage {
    /// The task failed before reporting that its required initialization completed.
    Starting,
    /// The task exited after it had reported readiness.
    Running,
}

impl RuntimeCriticalTaskFailureStage {
    /// Returns the stable structured-log value for this stage.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
        }
    }
}

/// Low-cardinality reason a critical runtime task terminated the process.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCriticalTaskFailureReason {
    /// The task did not report readiness before its configured deadline.
    ReadinessTimedOut,
    /// The task discarded its one-shot readiness signal without reporting readiness.
    ReadinessNotReported,
    /// The task returned successfully even though critical tasks must remain active.
    ExitedNormally,
    /// The task returned a typed operational failure.
    ExitedWithError {
        /// Low-cardinality failure kind returned by the task.
        kind: TaskExecutionErrorKind,
    },
    /// The task future was cancelled or unwound before returning a typed result.
    Aborted,
    /// The bounded supervisor event channel became unavailable unexpectedly.
    SupervisorUnavailable,
    /// The supervisor observed an impossible event ordering.
    ProtocolViolation,
}

impl RuntimeCriticalTaskFailureReason {
    /// Returns the stable structured-log value for this reason.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadinessTimedOut => "readiness_timed_out",
            Self::ReadinessNotReported => "readiness_not_reported",
            Self::ExitedNormally => "exited_normally",
            Self::ExitedWithError { .. } => "exited_with_error",
            Self::Aborted => "aborted",
            Self::SupervisorUnavailable => "supervisor_unavailable",
            Self::ProtocolViolation => "protocol_violation",
        }
    }
}

/// Failure reasons for reporting a critical task's one-shot readiness signal.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriticalTaskReadySignalErrorReason {
    /// The runtime stopped accepting lifecycle events.
    RuntimeUnavailable,
    /// The bounded lifecycle event channel had no remaining capacity.
    EventCapacityExceeded,
}

/// Typed failure returned when a critical task cannot report readiness.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("critical task readiness signal could not be reported")]
pub struct CriticalTaskReadySignalError {
    reason: CriticalTaskReadySignalErrorReason,
}

impl CriticalTaskReadySignalError {
    /// Returns the low-cardinality signaling failure reason.
    pub const fn reason(self) -> CriticalTaskReadySignalErrorReason {
        self.reason
    }
}

/// One-shot handle used by a critical task to release the runtime readiness barrier.
///
/// A task must retain this value until all initialization required for safe
/// service readiness has completed. Dropping it without calling
/// [`Self::mark_ready`] fails startup immediately instead of leaving the process
/// in an ambiguous partially initialized state.
pub struct CriticalTaskReadySignal {
    task_name: TaskName,
    sender: Option<mpsc::Sender<CriticalTaskEvent>>,
}

impl CriticalTaskReadySignal {
    /// Reports that the critical task is initialized and ready to remain active.
    pub fn mark_ready(mut self) -> Result<(), CriticalTaskReadySignalError> {
        let sender = match self.sender.take() {
            Some(sender) => sender,
            None => {
                return Err(CriticalTaskReadySignalError {
                    reason: CriticalTaskReadySignalErrorReason::RuntimeUnavailable,
                });
            }
        };

        send_event(
            &sender,
            CriticalTaskEvent::Ready {
                task_name: self.task_name.clone(),
            },
        )
    }
}

impl Drop for CriticalTaskReadySignal {
    fn drop(&mut self) {
        let Some(sender) = self.sender.take() else {
            return;
        };
        let _ = sender.try_send(CriticalTaskEvent::ReadinessNotReported {
            task_name: self.task_name.clone(),
        });
    }
}

/// Long-running task whose readiness and lifetime are required by the server process.
///
/// Unlike [`crate::runtime::RuntimeBackgroundTask`], a critical task blocks
/// global readiness until it explicitly reports successful initialization. Any
/// later return, operational error, cancellation, or unwind makes the runtime
/// not-ready, initiates coordinated shutdown, and causes the runtime to return
/// a typed failure.
pub struct RuntimeCriticalTask {
    task_name: TaskName,
    readiness_timeout: CriticalTaskReadinessTimeout,
    task: BoxedCriticalTaskFactory,
}

impl RuntimeCriticalTask {
    /// Creates a critical task with an explicit bounded readiness deadline.
    pub fn new<F, Fut>(
        task_name: TaskName,
        readiness_timeout: CriticalTaskReadinessTimeout,
        task: F,
    ) -> Self
    where
        F: FnOnce(ShutdownToken, CriticalTaskReadySignal) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), TaskExecutionError>> + Send + 'static,
    {
        Self {
            task_name,
            readiness_timeout,
            task: Box::new(move |shutdown, ready| Box::pin(task(shutdown, ready))),
        }
    }

    /// Creates a critical task using the stable name in an app descriptor.
    pub fn from_app_descriptor<F, Fut>(
        descriptor: &AppBackgroundTaskDescriptor,
        readiness_timeout: CriticalTaskReadinessTimeout,
        task: F,
    ) -> Result<Self, StartupError>
    where
        F: FnOnce(ShutdownToken, CriticalTaskReadySignal) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), TaskExecutionError>> + Send + 'static,
    {
        let task_name = TaskName::new(descriptor.name().as_str())?;
        Ok(Self::new(task_name, readiness_timeout, task))
    }
}

#[derive(Debug)]
pub(crate) struct CriticalTaskFailure {
    pub(crate) task_name: TaskName,
    pub(crate) stage: RuntimeCriticalTaskFailureStage,
    pub(crate) reason: RuntimeCriticalTaskFailureReason,
}

pub(crate) struct CriticalTaskMonitor {
    receiver: mpsc::Receiver<CriticalTaskEvent>,
    fallback_task_name: TaskName,
}

impl CriticalTaskMonitor {
    pub(crate) async fn next_failure(&mut self) -> CriticalTaskFailure {
        match self.receiver.recv().await {
            Some(CriticalTaskEvent::Exited { task_name, reason }) => CriticalTaskFailure {
                task_name,
                stage: RuntimeCriticalTaskFailureStage::Running,
                reason,
            },
            Some(CriticalTaskEvent::Ready { task_name })
            | Some(CriticalTaskEvent::ReadinessNotReported { task_name }) => CriticalTaskFailure {
                task_name,
                stage: RuntimeCriticalTaskFailureStage::Running,
                reason: RuntimeCriticalTaskFailureReason::ProtocolViolation,
            },
            None => CriticalTaskFailure {
                task_name: self.fallback_task_name.clone(),
                stage: RuntimeCriticalTaskFailureStage::Running,
                reason: RuntimeCriticalTaskFailureReason::SupervisorUnavailable,
            },
        }
    }
}

pub(crate) async fn start_critical_tasks(
    critical_tasks: Vec<RuntimeCriticalTask>,
    tasks: &mut BackgroundTaskSet,
    server_name: &ServerName,
) -> Result<Option<CriticalTaskMonitor>, CriticalTaskStartError> {
    let Some(first_task_name) = critical_tasks.first().map(|task| task.task_name.clone()) else {
        return Ok(None);
    };

    let event_capacity = MAX_BACKGROUND_TASK_CAPACITY
        .as_usize()
        .checked_mul(CRITICAL_TASK_EVENTS_PER_TASK)
        .ok_or_else(|| {
            CriticalTaskStartError::Failure(CriticalTaskFailure {
                task_name: first_task_name.clone(),
                stage: RuntimeCriticalTaskFailureStage::Starting,
                reason: RuntimeCriticalTaskFailureReason::SupervisorUnavailable,
            })
        })?;
    let (sender, mut receiver) = mpsc::channel(event_capacity);

    for critical_task in critical_tasks {
        let task_name = critical_task.task_name.clone();
        let readiness_timeout = critical_task.readiness_timeout;
        log_runtime_critical_task_started(server_name, &task_name);
        spawn_critical_task(critical_task, tasks, &sender)
            .map_err(CriticalTaskStartError::Registration)?;

        let startup_result = time::timeout(
            readiness_timeout.as_duration(),
            wait_for_task_readiness(&mut receiver, &task_name),
        )
        .await;

        match startup_result {
            Ok(Ok(())) => log_runtime_critical_task_ready(server_name, &task_name),
            Ok(Err(failure)) => return Err(CriticalTaskStartError::Failure(failure)),
            Err(_) => {
                return Err(CriticalTaskStartError::Failure(CriticalTaskFailure {
                    task_name,
                    stage: RuntimeCriticalTaskFailureStage::Starting,
                    reason: RuntimeCriticalTaskFailureReason::ReadinessTimedOut,
                }));
            }
        }
    }

    drop(sender);
    Ok(Some(CriticalTaskMonitor {
        receiver,
        fallback_task_name: first_task_name,
    }))
}

pub(crate) enum CriticalTaskStartError {
    Registration(ShutdownError),
    Failure(CriticalTaskFailure),
}

fn spawn_critical_task(
    critical_task: RuntimeCriticalTask,
    tasks: &mut BackgroundTaskSet,
    sender: &mpsc::Sender<CriticalTaskEvent>,
) -> Result<(), ShutdownError> {
    let RuntimeCriticalTask {
        task_name,
        readiness_timeout: _,
        task,
    } = critical_task;
    let task_name_for_signal = task_name.clone();
    let event_sender = sender.clone();

    tasks.spawn_fallible(task_name, move |shutdown| async move {
        let exit_guard =
            CriticalTaskExitGuard::new(task_name_for_signal.clone(), event_sender.clone());
        let ready = CriticalTaskReadySignal {
            task_name: task_name_for_signal,
            sender: Some(event_sender),
        };
        let result = task(shutdown, ready).await;
        let reason = match result {
            Ok(()) => RuntimeCriticalTaskFailureReason::ExitedNormally,
            Err(error) => RuntimeCriticalTaskFailureReason::ExitedWithError { kind: error.kind() },
        };
        exit_guard.report(reason);
        Ok(())
    })
}

async fn wait_for_task_readiness(
    receiver: &mut mpsc::Receiver<CriticalTaskEvent>,
    expected_task_name: &TaskName,
) -> Result<(), CriticalTaskFailure> {
    match receiver.recv().await {
        Some(CriticalTaskEvent::Ready { task_name }) if task_name == *expected_task_name => Ok(()),
        Some(CriticalTaskEvent::Ready { task_name }) => Err(CriticalTaskFailure {
            task_name,
            stage: RuntimeCriticalTaskFailureStage::Starting,
            reason: RuntimeCriticalTaskFailureReason::ProtocolViolation,
        }),
        Some(CriticalTaskEvent::ReadinessNotReported { task_name }) => Err(CriticalTaskFailure {
            task_name,
            stage: RuntimeCriticalTaskFailureStage::Starting,
            reason: RuntimeCriticalTaskFailureReason::ReadinessNotReported,
        }),
        Some(CriticalTaskEvent::Exited { task_name, reason }) => Err(CriticalTaskFailure {
            stage: if task_name == *expected_task_name {
                RuntimeCriticalTaskFailureStage::Starting
            } else {
                RuntimeCriticalTaskFailureStage::Running
            },
            task_name,
            reason,
        }),
        None => Err(CriticalTaskFailure {
            task_name: expected_task_name.clone(),
            stage: RuntimeCriticalTaskFailureStage::Starting,
            reason: RuntimeCriticalTaskFailureReason::SupervisorUnavailable,
        }),
    }
}

fn send_event(
    sender: &mpsc::Sender<CriticalTaskEvent>,
    event: CriticalTaskEvent,
) -> Result<(), CriticalTaskReadySignalError> {
    sender.try_send(event).map_err(|error| {
        let reason = match error {
            mpsc::error::TrySendError::Full(_) => {
                CriticalTaskReadySignalErrorReason::EventCapacityExceeded
            }
            mpsc::error::TrySendError::Closed(_) => {
                CriticalTaskReadySignalErrorReason::RuntimeUnavailable
            }
        };
        CriticalTaskReadySignalError { reason }
    })
}

enum CriticalTaskEvent {
    Ready {
        task_name: TaskName,
    },
    ReadinessNotReported {
        task_name: TaskName,
    },
    Exited {
        task_name: TaskName,
        reason: RuntimeCriticalTaskFailureReason,
    },
}

struct CriticalTaskExitGuard {
    task_name: TaskName,
    sender: Option<mpsc::Sender<CriticalTaskEvent>>,
}

impl CriticalTaskExitGuard {
    fn new(task_name: TaskName, sender: mpsc::Sender<CriticalTaskEvent>) -> Self {
        Self {
            task_name,
            sender: Some(sender),
        }
    }

    fn report(mut self, reason: RuntimeCriticalTaskFailureReason) {
        let Some(sender) = self.sender.take() else {
            return;
        };
        let _ = sender.try_send(CriticalTaskEvent::Exited {
            task_name: self.task_name.clone(),
            reason,
        });
    }
}

impl Drop for CriticalTaskExitGuard {
    fn drop(&mut self) {
        let Some(sender) = self.sender.take() else {
            return;
        };
        let _ = sender.try_send(CriticalTaskEvent::Exited {
            task_name: self.task_name.clone(),
            reason: RuntimeCriticalTaskFailureReason::Aborted,
        });
    }
}

pub(crate) fn log_critical_task_failure(server_name: &ServerName, failure: &CriticalTaskFailure) {
    log_runtime_critical_task_failed(
        server_name,
        &failure.task_name,
        failure.stage,
        failure.reason,
    );
}
