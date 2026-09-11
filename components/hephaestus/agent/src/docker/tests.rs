// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_hephaestus_domain::{
    HephaestusContainerHealthState, HephaestusContainerRestartReason, HephaestusContainerState,
};

use super::{
    collect_command_first_line, container_report_from_summary, docker_engine,
    docker_socket_available_at,
};

#[test]
fn maps_engine_container_summary() {
    let row = docker_engine::DockerContainerSummaryForTest::new(
        vec!["/reallyme-nats".to_owned()],
        "ams.vultrcr.com/reallyme/nats:2.14.0".to_owned(),
        "running".to_owned(),
        "Up 10 seconds (healthy)".to_owned(),
    );

    let report = container_report_from_summary(row.as_summary())
        .unwrap_or_else(|error| panic!("valid summary: {error:?}"))
        .unwrap_or_else(|| panic!("summary has name"));

    assert_eq!(report.state(), HephaestusContainerState::Running);
    assert_eq!(report.health(), HephaestusContainerHealthState::Healthy);
    assert!(!report.restart_required());
    assert_eq!(
        report.restart_reason(),
        HephaestusContainerRestartReason::None
    );
}

#[test]
fn marks_unhealthy_container_as_requiring_restart() {
    let row = docker_engine::DockerContainerSummaryForTest::new(
        vec!["/reallyme-api".to_owned()],
        "ams.vultrcr.com/reallyme/api:2026.05.21".to_owned(),
        "running".to_owned(),
        "Up 10 seconds (unhealthy)".to_owned(),
    );

    let report = container_report_from_summary(row.as_summary())
        .unwrap_or_else(|error| panic!("valid summary: {error:?}"))
        .unwrap_or_else(|| panic!("summary has name"));

    assert!(report.restart_required());
    assert_eq!(
        report.restart_reason(),
        HephaestusContainerRestartReason::Unhealthy
    );
}

#[test]
fn marks_terminal_container_state_as_requiring_restart() {
    let row = docker_engine::DockerContainerSummaryForTest::new(
        vec!["/reallyme-worker".to_owned()],
        "ams.vultrcr.com/reallyme/worker:2026.05.21".to_owned(),
        "exited".to_owned(),
        "Exited (1) 2 minutes ago".to_owned(),
    );

    let report = container_report_from_summary(row.as_summary())
        .unwrap_or_else(|error| panic!("valid summary: {error:?}"))
        .unwrap_or_else(|| panic!("summary has name"));

    assert!(report.restart_required());
    assert_eq!(
        report.restart_reason(),
        HephaestusContainerRestartReason::Exited
    );
}

#[tokio::test]
async fn missing_optional_runtime_command_returns_none() {
    let version = collect_command_first_line(
        "/definitely/missing/reallyme/docker",
        &["compose", "version", "--short"],
        std::time::Duration::from_secs(1),
    )
    .await
    .unwrap_or_else(|error| panic!("missing optional command should not fail: {error:?}"));

    assert!(version.is_none());
}

#[tokio::test]
async fn missing_docker_socket_is_not_fatal_for_non_docker_hosts() {
    let available = docker_socket_available_at("/definitely/missing/reallyme/docker.sock")
        .await
        .unwrap_or_else(|error| panic!("missing docker socket should not fail: {error:?}"));

    assert!(!available);
}
