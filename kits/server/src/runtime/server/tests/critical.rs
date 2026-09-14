// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::{self, Future};
use std::time::Duration;

use axum::Router;
use tokio::sync::oneshot;
use tokio::time;

use super::fixtures::{
    critical_task_regression_action, observability_config, spawn_critical_task_regression_worker,
};
use crate::health::{Readiness, ReadinessState};
use crate::runtime::{
    AppName, CriticalTaskReadinessTimeout, CriticalTaskReadinessTimeoutErrorReason, RuntimeApp,
    RuntimeBackgroundTask, RuntimeCleanupHook, RuntimeCriticalTask, ServerRuntime,
    ServerRuntimeBuilder, ServerRuntimeError, ServerRuntimePhase, ServerRuntimePhaseReporter,
};
use crate::shutdown::{ShutdownError, ShutdownReason};
use crate::startup::{ServerName, TaskName};
use crate::task::{ShutdownTimeout, TaskExecutionError, TaskExecutionErrorKind};
use crate::version::BuildInfo;

const WORKER_TIMEOUT: Duration = Duration::from_secs(10);
const STARTUP_TIMEOUT: Duration = Duration::from_millis(250);

#[test]
fn critical_task_readiness_timeout_is_strictly_bounded() {
    let zero = CriticalTaskReadinessTimeout::new(Duration::ZERO)
        .expect_err("zero timeout must fail validation");
    let oversized = CriticalTaskReadinessTimeout::new(Duration::from_secs(3_601))
        .expect_err("oversized timeout must fail validation");

    assert_eq!(
        zero.reason(),
        CriticalTaskReadinessTimeoutErrorReason::MustBeGreaterThanZero
    );
    assert_eq!(
        oversized.reason(),
        CriticalTaskReadinessTimeoutErrorReason::ExceedsMaximum
    );
}

#[test]
fn critical_task_readiness_barrier_blocks_global_readiness() {
    assert_regression_worker_succeeds("readiness-barrier");
}

#[test]
fn critical_task_normal_exit_terminates_runtime_and_drains_tasks() {
    assert_regression_worker_succeeds("normal-exit");
}

#[test]
fn critical_task_typed_failure_is_preserved() {
    assert_regression_worker_succeeds("typed-failure");
}

#[test]
fn critical_task_startup_timeout_fails_closed() {
    assert_regression_worker_succeeds("startup-timeout");
}

#[test]
fn dropped_critical_task_readiness_signal_fails_closed() {
    assert_regression_worker_succeeds("readiness-not-reported");
}

#[test]
fn critical_task_unwind_is_detected() {
    assert_regression_worker_succeeds("task-unwind");
}

#[test]
fn critical_task_regression_subprocess_worker() {
    let Some(action) = critical_task_regression_action() else {
        return;
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("worker tokio runtime should build")
        .block_on(async move {
            match action.to_string_lossy().as_ref() {
                "readiness-barrier" => readiness_barrier_worker().await,
                "normal-exit" => normal_exit_worker().await,
                "typed-failure" => typed_failure_worker().await,
                "startup-timeout" => startup_timeout_worker().await,
                "readiness-not-reported" => readiness_not_reported_worker().await,
                "task-unwind" => task_unwind_worker().await,
                value => panic!("unsupported critical task regression action: {value}"),
            }
        });
}

fn assert_regression_worker_succeeds(action: &'static str) {
    let output = spawn_critical_task_regression_worker(action);

    assert!(
        output.status.success(),
        "critical task regression subprocess failed for {action}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn runtime_builder(
    server_name: &'static str,
    readiness: Readiness,
    phase_reporter: ServerRuntimePhaseReporter,
) -> ServerRuntimeBuilder {
    let server_name = ServerName::new(server_name).expect("valid fixture server name");

    ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(observability_config())
        .build_info(BuildInfo::new(server_name))
        .readiness(readiness)
        .shutdown_timeout(
            ShutdownTimeout::new(Duration::from_secs(1)).expect("valid fixture shutdown timeout"),
        )
        .cleanup_timeout(
            ShutdownTimeout::new(Duration::from_secs(1)).expect("valid fixture cleanup timeout"),
        )
        .phase_reporter(phase_reporter)
}

fn critical_task<F, Fut>(task_name: &'static str, task: F) -> RuntimeCriticalTask
where
    F: FnOnce(crate::task::ShutdownToken, crate::runtime::CriticalTaskReadySignal) -> Fut
        + Send
        + 'static,
    Fut: Future<Output = Result<(), TaskExecutionError>> + Send + 'static,
{
    RuntimeCriticalTask::new(
        TaskName::new(task_name).expect("valid fixture task name"),
        CriticalTaskReadinessTimeout::new(STARTUP_TIMEOUT).expect("valid fixture startup timeout"),
        task,
    )
}

fn map_ready_error(
    result: Result<(), crate::runtime::CriticalTaskReadySignalError>,
) -> Result<(), TaskExecutionError> {
    result.map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))
}

async fn readiness_barrier_worker() {
    let readiness = Readiness::new();
    let mut readiness_watch = readiness.watch();
    let (phase_reporter, phase_watch) = ServerRuntimePhaseReporter::new();
    let (release_sender, release_receiver) = oneshot::channel();
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();

    let runtime = runtime_builder(
        "critical-barrier-fixture",
        readiness.clone(),
        phase_reporter,
    )
    .critical_task(critical_task(
        "state-reconciler",
        move |mut shutdown, ready| async move {
            release_receiver
                .await
                .map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))?;
            map_ready_error(ready.mark_ready())?;
            let _ = shutdown.cancelled().await;
            Ok(())
        },
    ))
    .build()
    .expect("runtime should build");

    let runtime_task = tokio::spawn(runtime.run_until_shutdown(async move {
        shutdown_receiver.await.map_err(|_| {
            crate::shutdown::ShutdownError::ListenerInstallFailed {
                kind: crate::shutdown::ShutdownSignalKind::CtrlC,
            }
        })?;
        Ok(ShutdownReason::Sigterm)
    }));

    assert!(
        time::timeout(Duration::from_millis(50), readiness_watch.changed())
            .await
            .is_err(),
        "global readiness must remain blocked"
    );
    release_sender
        .send(())
        .expect("critical task should await release");
    assert_eq!(
        time::timeout(Duration::from_secs(1), readiness_watch.changed())
            .await
            .expect("readiness transition should be bounded")
            .expect("readiness publisher should remain open"),
        ReadinessState::Ready
    );
    shutdown_sender
        .send(())
        .expect("runtime should await shutdown");
    runtime_task
        .await
        .expect("runtime task should not panic")
        .expect("runtime should shut down cleanly");
    assert_eq!(phase_watch.current(), ServerRuntimePhase::Stopped);
}

async fn normal_exit_worker() {
    let readiness = Readiness::new();
    let mut readiness_watch = readiness.watch();
    let (phase_reporter, phase_watch) = ServerRuntimePhaseReporter::new();
    let (exit_sender, exit_receiver) = oneshot::channel();
    let (shutdown_observed_sender, shutdown_observed_receiver) = oneshot::channel();
    let (cleanup_sender, cleanup_receiver) = oneshot::channel();

    let companion = RuntimeBackgroundTask::new(
        TaskName::new("checkpoint-writer").expect("valid fixture task name"),
        move |mut shutdown| async move {
            let reason = shutdown.cancelled().await;
            let _ = shutdown_observed_sender.send(reason);
            Ok(())
        },
    );
    let app = RuntimeApp::new(
        AppName::new("critical-fixture").expect("valid fixture app name"),
        Router::new(),
    )
    .with_critical_task(critical_task(
        "event-consumer",
        move |_shutdown, ready| async move {
            map_ready_error(ready.mark_ready())?;
            exit_receiver
                .await
                .map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))?;
            Ok(())
        },
    ))
    .with_cleanup_hook(RuntimeCleanupHook::new(
        TaskName::new("flush-state").expect("valid fixture task name"),
        move || async move {
            let _ = cleanup_sender.send(());
            Ok(())
        },
    ));
    let runtime = runtime_builder("critical-exit-fixture", readiness.clone(), phase_reporter)
        .background_task(companion)
        .app(app)
        .build()
        .expect("runtime should build");

    let runtime_task = tokio::spawn(runtime.run_until_shutdown(future::pending()));
    wait_until_ready(&mut readiness_watch).await;
    exit_sender
        .send(())
        .expect("critical task should await exit trigger");

    let result = time::timeout(WORKER_TIMEOUT, runtime_task)
        .await
        .expect("runtime termination should be bounded")
        .expect("runtime task should not panic");
    assert_critical_failure(result, TaskExecutionErrorKind::Internal);
    assert_eq!(
        shutdown_observed_receiver
            .await
            .expect("companion task should observe shutdown"),
        ShutdownReason::Unknown
    );
    cleanup_receiver
        .await
        .expect("app cleanup hook should execute");
    assert!(!readiness.is_ready());
    assert_eq!(phase_watch.current(), ServerRuntimePhase::Failed);
}

async fn typed_failure_worker() {
    let readiness = Readiness::new();
    let mut readiness_watch = readiness.watch();
    let (phase_reporter, _) = ServerRuntimePhaseReporter::new();
    let (exit_sender, exit_receiver) = oneshot::channel();
    let runtime = runtime_builder("critical-error-fixture", readiness, phase_reporter)
        .critical_task(critical_task(
            "lease-renewer",
            move |_shutdown, ready| async move {
                map_ready_error(ready.mark_ready())?;
                exit_receiver
                    .await
                    .map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))?;
                Err(TaskExecutionError::new(
                    TaskExecutionErrorKind::DependencyUnavailable,
                ))
            },
        ))
        .build()
        .expect("runtime should build");
    let runtime_task = tokio::spawn(runtime.run_until_shutdown(future::pending()));
    wait_until_ready(&mut readiness_watch).await;
    exit_sender
        .send(())
        .expect("critical task should await exit trigger");
    let result = runtime_task.await.expect("runtime task should not panic");
    assert_critical_failure(result, TaskExecutionErrorKind::DependencyUnavailable);
}

async fn startup_timeout_worker() {
    let readiness = Readiness::new();
    let (phase_reporter, _) = ServerRuntimePhaseReporter::new();
    let runtime = runtime_builder(
        "critical-timeout-fixture",
        readiness.clone(),
        phase_reporter,
    )
    .critical_task(critical_task(
        "cache-loader",
        |mut shutdown, ready| async move {
            let _ready = ready;
            let _ = shutdown.cancelled().await;
            Ok(())
        },
    ))
    .build()
    .expect("runtime should build");
    let result = runtime.run_until_shutdown(future::pending()).await;

    assert_critical_failure(result, TaskExecutionErrorKind::Internal);
    assert!(!readiness.is_ready());
}

async fn readiness_not_reported_worker() {
    let readiness = Readiness::new();
    let (phase_reporter, _) = ServerRuntimePhaseReporter::new();
    let runtime = runtime_builder("critical-signal-fixture", readiness, phase_reporter)
        .critical_task(critical_task(
            "snapshot-loader",
            |mut shutdown, ready| async move {
                drop(ready);
                let _ = shutdown.cancelled().await;
                Ok(())
            },
        ))
        .build()
        .expect("runtime should build");
    let result = runtime.run_until_shutdown(future::pending()).await;

    assert_critical_failure(result, TaskExecutionErrorKind::Internal);
}

async fn task_unwind_worker() {
    let readiness = Readiness::new();
    let (phase_reporter, _) = ServerRuntimePhaseReporter::new();
    let runtime = runtime_builder("critical-unwind-fixture", readiness, phase_reporter)
        .critical_task(critical_task(
            "journal-reader",
            |_shutdown, ready| async move {
                map_ready_error(ready.mark_ready())?;
                panic!("intentional critical task unwind fixture");
            },
        ))
        .build()
        .expect("runtime should build");
    let result = runtime.run_until_shutdown(future::pending()).await;

    assert_critical_failure(result, TaskExecutionErrorKind::Internal);
}

async fn wait_until_ready(readiness_watch: &mut crate::health::ReadinessWatcher) {
    assert_eq!(
        time::timeout(Duration::from_secs(1), readiness_watch.changed())
            .await
            .expect("readiness transition should be bounded")
            .expect("readiness publisher should remain open"),
        ReadinessState::Ready
    );
}

fn assert_critical_failure(
    result: Result<(), ServerRuntimeError>,
    expected_kind: TaskExecutionErrorKind,
) {
    assert!(matches!(
        result,
        Err(ServerRuntimeError::Shutdown {
            source: ShutdownError::TaskExitedWithError { kind, .. },
        }) if kind == expected_kind
    ));
}
