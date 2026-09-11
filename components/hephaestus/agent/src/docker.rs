// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Docker observation through the Docker Engine API plus fixed Compose version checks.

use std::io::ErrorKind;
use std::os::unix::fs::FileTypeExt;
use std::sync::OnceLock;
use std::time::Duration;

use reallyme_hephaestus_domain::{
    DockerContainerName, DockerImageReference, HephaestusAgentReportLine,
    HephaestusContainerHealthState, HephaestusContainerReport, HephaestusContainerRestartReason,
    HephaestusContainerState,
};
use tokio::process::Command;
use tokio::time::timeout;

use crate::docker_engine;
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

const DOCKER_BINARY_PATH: &str = "/usr/bin/docker";
static DOCKER_COMPOSE_VERSION: OnceLock<Option<String>> = OnceLock::new();

/// Docker and Compose versions observed locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerRuntimeVersions {
    docker_version: Option<String>,
    docker_compose_version: Option<String>,
}

impl DockerRuntimeVersions {
    /// Constructs a runtime version snapshot.
    pub const fn new(
        docker_version: Option<String>,
        docker_compose_version: Option<String>,
    ) -> Self {
        Self {
            docker_version,
            docker_compose_version,
        }
    }

    /// Returns the Docker Engine version.
    pub fn docker_version(&self) -> Option<&str> {
        self.docker_version.as_deref()
    }

    /// Returns the Docker Compose plugin version.
    pub fn docker_compose_version(&self) -> Option<&str> {
        self.docker_compose_version.as_deref()
    }
}

/// Collects Docker container state using fixed arguments and no shell.
pub async fn collect_containers(
    command_timeout: Duration,
) -> AgentResult<Vec<HephaestusContainerReport>> {
    if !docker_socket_available().await? {
        return Ok(Vec::new());
    }
    let containers = docker_engine::list_containers(command_timeout).await?;
    containers
        .into_iter()
        .filter_map(|container| container_report_from_summary(&container).transpose())
        .collect()
}

/// Collects Docker Engine and Compose plugin versions.
pub async fn collect_runtime_versions(
    command_timeout: Duration,
) -> AgentResult<DockerRuntimeVersions> {
    if !docker_socket_available().await? {
        return Ok(DockerRuntimeVersions::new(None, None));
    }
    let docker_version = docker_engine::docker_version(command_timeout).await?;
    let docker_compose_version = if let Some(value) = DOCKER_COMPOSE_VERSION.get() {
        value.clone()
    } else {
        let version = collect_command_first_line(
            DOCKER_BINARY_PATH,
            &["compose", "version", "--short"],
            command_timeout,
        )
        .await?;
        if version.is_some() {
            let _ = DOCKER_COMPOSE_VERSION.set(version.clone());
        }
        version
    };
    Ok(DockerRuntimeVersions::new(
        docker_version,
        docker_compose_version,
    ))
}

async fn docker_socket_available() -> AgentResult<bool> {
    docker_socket_available_at(docker_engine::DOCKER_SOCKET_PATH).await
}

async fn docker_socket_available_at(path: &str) -> AgentResult<bool> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.file_type().is_socket()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_error) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
    }
}

async fn collect_command_first_line(
    executable: &str,
    args: &[&str],
    command_timeout: Duration,
) -> AgentResult<Option<String>> {
    let mut command = Command::new(executable);
    command.args(args);
    let output = timeout(command_timeout, crate::command::output(&mut command))
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    let output = match output {
        Ok(output) => output,
        Err(error) if error.reason() == HephaestusAgentErrorReason::CommandUnavailable => {
            return Ok(None);
        }
        Err(_error) => {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::CommandFailed,
            ));
        }
    };
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8(output.stdout).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })?;
    Ok(stdout
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned))
}

fn container_report_from_summary(
    container: &docker_engine::DockerContainerSummary,
) -> AgentResult<Option<HephaestusContainerReport>> {
    let Some(name) = container.primary_name() else {
        return Ok(None);
    };
    let name = DockerContainerName::new(name).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::DomainValidationFailed)
    })?;
    let image = DockerImageReference::new(container.image()).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::DomainValidationFailed)
    })?;
    let state = container_state(container.state());
    let health = container_health(container.status());
    let restart_reason = container_restart_reason(state, health);
    HephaestusContainerReport::new(
        name,
        image,
        state,
        health,
        Vec::<HephaestusAgentReportLine>::new(),
        restart_reason != HephaestusContainerRestartReason::None,
        restart_reason,
    )
    .map(Some)
    .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::DomainValidationFailed))
}

fn container_state(value: &str) -> HephaestusContainerState {
    match value {
        "created" | "Created" => HephaestusContainerState::Created,
        "running" | "Running" => HephaestusContainerState::Running,
        "restarting" | "Restarting" => HephaestusContainerState::Restarting,
        "removing" | "Removing" => HephaestusContainerState::Removing,
        "paused" | "Paused" => HephaestusContainerState::Paused,
        "exited" | "Exited" => HephaestusContainerState::Exited,
        "dead" | "Dead" => HephaestusContainerState::Dead,
        _ => HephaestusContainerState::Unknown,
    }
}

fn container_health(status: &str) -> HephaestusContainerHealthState {
    if status.contains("(healthy)") {
        HephaestusContainerHealthState::Healthy
    } else if status.contains("(unhealthy)") {
        HephaestusContainerHealthState::Unhealthy
    } else if status.contains("(health: starting)") {
        HephaestusContainerHealthState::Starting
    } else {
        HephaestusContainerHealthState::None
    }
}

fn container_restart_reason(
    state: HephaestusContainerState,
    health: HephaestusContainerHealthState,
) -> HephaestusContainerRestartReason {
    match (state, health) {
        (_, HephaestusContainerHealthState::Unhealthy) => {
            HephaestusContainerRestartReason::Unhealthy
        }
        (HephaestusContainerState::Restarting, _) => HephaestusContainerRestartReason::Restarting,
        (HephaestusContainerState::Exited, _) => HephaestusContainerRestartReason::Exited,
        (HephaestusContainerState::Dead, _) => HephaestusContainerRestartReason::Dead,
        (HephaestusContainerState::Unknown, _) | (_, HephaestusContainerHealthState::Unknown) => {
            HephaestusContainerRestartReason::Unknown
        }
        (
            HephaestusContainerState::Created
            | HephaestusContainerState::Running
            | HephaestusContainerState::Removing
            | HephaestusContainerState::Paused,
            HephaestusContainerHealthState::Healthy
            | HephaestusContainerHealthState::Starting
            | HephaestusContainerHealthState::None,
        ) => HephaestusContainerRestartReason::None,
    }
}

#[cfg(test)]
mod tests;
