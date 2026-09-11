// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{DOCKER_BINARY, DOCKER_CONFIG_ENV_VAR, ResolvedRegistryAuth};
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};
use secrecy::ExposeSecret;
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

pub(super) async fn docker_login(
    auth: &ResolvedRegistryAuth,
    docker_config_dir: &Path,
    command_timeout: Duration,
) -> AgentResult<()> {
    let mut command = Command::new(DOCKER_BINARY);
    command
        .kill_on_drop(true)
        .env(DOCKER_CONFIG_ENV_VAR, docker_config_dir)
        .arg("login")
        .arg(auth.registry.as_str())
        .arg("--username")
        .arg(auth.username.expose_secret())
        .arg("--password-stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    let result = timeout(command_timeout, async {
        stdin
            .write_all(auth.password.expose_secret().as_bytes())
            .await
            .map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed)
            })?;
        drop(stdin);
        child
            .wait()
            .await
            .map_err(|_| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))
    })
    .await;
    match result {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(_status)) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
        Ok(Err(_error)) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
        Err(_error) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
    }
}
