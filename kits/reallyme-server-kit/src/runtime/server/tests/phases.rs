// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::{Ipv4Addr, TcpListener as StdTcpListener};
use std::time::Duration;

use axum::Router;

use super::fixtures::{
    http_server_config_for_port, observability_config, phase_regression_action,
    spawn_phase_regression_worker,
};
use crate::health::{Readiness, ReadinessState};
use crate::runtime::{
    HttpServerSpec, RuntimeStartupCheck, ServerRuntime, ServerRuntimeError, ServerRuntimePhase,
    ServerRuntimePhaseReporter, ServerRuntimeTransport,
};
use crate::shutdown::ShutdownReason;
use crate::startup::{ServerName, TaskName};
use crate::task::{ShutdownTimeout, TaskExecutionError};
use crate::version::BuildInfo;

#[test]
fn runtime_phase_ordering_is_observable_in_subprocess() {
    let output = spawn_phase_regression_worker("phase-order");

    assert!(
        output.status.success(),
        "phase regression subprocess failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn failed_listener_bind_moves_runtime_to_failed_phase() {
    let output = spawn_phase_regression_worker("bind-failure");

    assert!(
        output.status.success(),
        "phase failure subprocess failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn phase_regression_subprocess_worker() {
    let Some(action) = phase_regression_action() else {
        return;
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("worker tokio runtime should build")
        .block_on(async move {
            match action.to_string_lossy().as_ref() {
                "phase-order" => runtime_phase_order_worker().await,
                "bind-failure" => failed_bind_phase_worker().await,
                value => panic!("unsupported phase regression action: {value}"),
            }
        });
}

async fn runtime_phase_order_worker() {
    let server_name = ServerName::new("runtime-phase-order-regression").expect("valid server name");
    let readiness = Readiness::new();
    let mut readiness_watcher = readiness.watch();
    let (phase_reporter, mut phase_watcher) = ServerRuntimePhaseReporter::new();
    let runtime = ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(observability_config())
        .build_info(BuildInfo::new(server_name))
        .readiness(readiness.clone())
        .shutdown_timeout(
            ShutdownTimeout::new(Duration::from_secs(1)).expect("valid fixture shutdown timeout"),
        )
        .startup_check(RuntimeStartupCheck::new(
            TaskName::new("phase-order-startup-check").expect("valid fixture task name"),
            || async { Ok::<(), TaskExecutionError>(()) },
        ))
        .phase_reporter(phase_reporter)
        .build()
        .expect("runtime should build from valid fixtures");

    assert_eq!(phase_watcher.current(), ServerRuntimePhase::Initializing);
    assert!(!readiness.is_ready());

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let runtime_task = tokio::spawn(runtime.run_until_shutdown(async move {
        shutdown_receiver
            .await
            .expect("test shutdown trigger should be sent");
        Ok(ShutdownReason::Sigterm)
    }));

    assert_eq!(
        phase_watcher
            .next_transition()
            .await
            .expect("binding phase should be emitted"),
        ServerRuntimePhase::BindingListeners
    );
    assert_eq!(
        phase_watcher
            .next_transition()
            .await
            .expect("task-start phase should be emitted"),
        ServerRuntimePhase::StartingBackgroundTasks
    );
    assert_eq!(
        phase_watcher
            .next_transition()
            .await
            .expect("serving phase should be emitted"),
        ServerRuntimePhase::Serving
    );
    assert_eq!(
        readiness_watcher.changed().await.unwrap(),
        ReadinessState::Ready
    );

    shutdown_sender
        .send(())
        .expect("runtime should still be waiting for shutdown");

    assert_eq!(
        phase_watcher
            .next_transition()
            .await
            .expect("draining phase should be emitted"),
        ServerRuntimePhase::Draining
    );
    assert!(!readiness.is_ready());
    assert_eq!(
        phase_watcher
            .next_transition()
            .await
            .expect("shutting-down phase should be emitted"),
        ServerRuntimePhase::ShuttingDown
    );
    assert!(!readiness.is_ready());
    assert_eq!(
        phase_watcher
            .next_transition()
            .await
            .expect("stopped phase should be emitted"),
        ServerRuntimePhase::Stopped
    );

    runtime_task
        .await
        .expect("runtime task should not panic")
        .expect("runtime should shut down cleanly");
    assert_eq!(phase_watcher.current(), ServerRuntimePhase::Stopped);
}

async fn failed_bind_phase_worker() {
    let occupied_listener = match StdTcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(error) => panic!("test should bind occupied listener: {error}"),
    };
    let occupied_port = occupied_listener
        .local_addr()
        .expect("occupied listener should expose address")
        .port();
    let server_name =
        ServerName::new("runtime-phase-failure-regression").expect("valid server name");
    let (phase_reporter, mut phase_watcher) = ServerRuntimePhaseReporter::new();
    let runtime = ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(observability_config())
        .build_info(BuildInfo::new(server_name))
        .readiness(Readiness::new())
        .shutdown_timeout(
            ShutdownTimeout::new(Duration::from_secs(1)).expect("valid fixture shutdown timeout"),
        )
        .http_server(HttpServerSpec::new(
            http_server_config_for_port(occupied_port),
            Router::new(),
        ))
        .phase_reporter(phase_reporter)
        .build()
        .expect("runtime should build from valid fixtures");

    let result = runtime.run_until_shutdown(std::future::pending()).await;

    assert!(matches!(
        result,
        Err(ServerRuntimeError::ListenerBind {
            transport: ServerRuntimeTransport::Http,
            ..
        })
    ));
    assert_eq!(
        phase_watcher
            .next_transition()
            .await
            .expect("binding phase should be emitted"),
        ServerRuntimePhase::BindingListeners
    );
    assert_eq!(
        phase_watcher
            .next_transition()
            .await
            .expect("failed phase should be emitted"),
        ServerRuntimePhase::Failed
    );
    assert_eq!(phase_watcher.current(), ServerRuntimePhase::Failed);
}
