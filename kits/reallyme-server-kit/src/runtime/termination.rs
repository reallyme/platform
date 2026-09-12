// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;

use crate::health::{Readiness, ReadinessState};
use crate::observability::{
    ErrorKind, log_error, log_runtime_phase_transition, log_shutdown_completed,
    log_shutdown_requested, record_readiness_state, record_runtime_phase,
};
use crate::shutdown::{ShutdownError, ShutdownReason};
use crate::startup::ServerName;
use crate::task::{BackgroundTaskSet, ShutdownTimeout};

use super::app::RuntimeAppCleanup;
use super::cleanup::run_cleanup_hooks;
use super::critical::{CriticalTaskFailure, CriticalTaskMonitor, log_critical_task_failure};
use super::error::ServerRuntimeError;
use super::phase::{ServerRuntimePhase, ServerRuntimePhaseReporter};

pub(crate) enum RuntimeTermination {
    Shutdown(ShutdownReason),
    CriticalTask(CriticalTaskFailure),
}

pub(crate) struct CriticalTerminationContext<'a> {
    pub(crate) server_name: &'a ServerName,
    pub(crate) readiness: &'a Readiness,
    pub(crate) phase_reporter: &'a ServerRuntimePhaseReporter,
    pub(crate) tasks: &'a mut BackgroundTaskSet,
    pub(crate) cleanup_hooks: Vec<RuntimeAppCleanup>,
    pub(crate) shutdown_timeout: ShutdownTimeout,
    pub(crate) cleanup_timeout: ShutdownTimeout,
}

pub(crate) async fn wait_for_runtime_termination<F>(
    shutdown_signal_future: F,
    critical_task_monitor: &mut Option<CriticalTaskMonitor>,
) -> Result<RuntimeTermination, ServerRuntimeError>
where
    F: Future<Output = Result<ShutdownReason, ShutdownError>>,
{
    let Some(monitor) = critical_task_monitor.as_mut() else {
        return shutdown_signal_future
            .await
            .map(RuntimeTermination::Shutdown)
            .map_err(|source| ServerRuntimeError::Shutdown { source });
    };

    tokio::select! {
        biased;
        shutdown = shutdown_signal_future => shutdown
            .map(RuntimeTermination::Shutdown)
            .map_err(|source| ServerRuntimeError::Shutdown { source }),
        failure = monitor.next_failure() => Ok(RuntimeTermination::CriticalTask(failure)),
    }
}

pub(crate) async fn terminate_for_critical_task(
    failure: CriticalTaskFailure,
    context: CriticalTerminationContext<'_>,
) -> ServerRuntimeError {
    log_critical_task_failure(context.server_name, &failure);
    let task_error_kind = match failure.reason {
        super::critical::RuntimeCriticalTaskFailureReason::ExitedWithError { kind } => kind,
        super::critical::RuntimeCriticalTaskFailureReason::ReadinessTimedOut
        | super::critical::RuntimeCriticalTaskFailureReason::ReadinessNotReported
        | super::critical::RuntimeCriticalTaskFailureReason::ExitedNormally
        | super::critical::RuntimeCriticalTaskFailureReason::Aborted
        | super::critical::RuntimeCriticalTaskFailureReason::SupervisorUnavailable
        | super::critical::RuntimeCriticalTaskFailureReason::ProtocolViolation => {
            crate::task::TaskExecutionErrorKind::Internal
        }
    };
    // `ServerRuntimeError` and `ShutdownReason` were exhaustive public enums in
    // 0.1.0. Reusing their existing managed-task/unknown categories preserves
    // patch-release compatibility; the preceding structured event retains the
    // precise critical-task stage and reason for operators.
    let error = ServerRuntimeError::Shutdown {
        source: ShutdownError::TaskExitedWithError {
            task_name: failure.task_name,
            kind: task_error_kind,
        },
    };

    context.readiness.mark_not_ready();
    record_readiness_state(ReadinessState::NotReady);
    log_shutdown_requested(context.server_name, ShutdownReason::Unknown);
    transition_runtime_phase(
        context.phase_reporter,
        context.server_name,
        ServerRuntimePhase::Draining,
    );

    if context
        .tasks
        .shutdown(ShutdownReason::Unknown, context.shutdown_timeout)
        .await
        .is_err()
    {
        log_error(
            ErrorKind::Internal,
            "managed task drain failed after critical task termination",
            None,
            None,
        );
    }

    if run_cleanup_hooks(
        context.cleanup_hooks,
        context.cleanup_timeout,
        context.server_name,
    )
    .await
    .is_err()
    {
        log_error(
            ErrorKind::Internal,
            "app cleanup failed after critical task termination",
            None,
            None,
        );
    }

    transition_runtime_phase(
        context.phase_reporter,
        context.server_name,
        ServerRuntimePhase::ShuttingDown,
    );
    log_shutdown_completed(context.server_name, ShutdownReason::Unknown);

    error
}

fn transition_runtime_phase(
    phase_reporter: &ServerRuntimePhaseReporter,
    server_name: &ServerName,
    phase: ServerRuntimePhase,
) {
    phase_reporter.transition(phase);
    log_runtime_phase_transition(server_name, phase);
    record_runtime_phase(phase);
}
