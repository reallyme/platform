// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! systemd observation through fixed CLI calls.

use std::time::Duration;

use reallyme_hephaestus_domain::{
    HephaestusAgentReportLine, HephaestusHostServiceReport, HephaestusHostServiceState,
    HephaestusHostServiceUnitName,
};
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

/// Collects configured systemd unit states.
pub async fn collect_systemd_units(
    units: &[String],
    command_timeout: Duration,
) -> AgentResult<Vec<HephaestusHostServiceReport>> {
    let mut reports = Vec::new();
    for unit in units {
        reports.push(collect_one_unit(unit.as_str(), command_timeout).await?);
    }
    Ok(reports)
}

async fn collect_one_unit(
    unit: &str,
    command_timeout: Duration,
) -> AgentResult<HephaestusHostServiceReport> {
    let mut command = Command::new("/bin/systemctl");
    command.args(["is-active", unit]);
    let output = timeout(command_timeout, crate::command::output(&mut command))
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    let stdout = String::from_utf8(output.stdout).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })?;
    let unit_name = HephaestusHostServiceUnitName::new(unit).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::DomainValidationFailed)
    })?;
    HephaestusHostServiceReport::new(
        unit_name,
        map_systemd_state(stdout.trim()),
        Vec::<HephaestusAgentReportLine>::new(),
    )
    .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::DomainValidationFailed))
}

fn map_systemd_state(value: &str) -> HephaestusHostServiceState {
    match value {
        "active" => HephaestusHostServiceState::Active,
        "reloading" => HephaestusHostServiceState::Reloading,
        "inactive" => HephaestusHostServiceState::Inactive,
        "failed" => HephaestusHostServiceState::Failed,
        "activating" => HephaestusHostServiceState::Activating,
        "deactivating" => HephaestusHostServiceState::Deactivating,
        _ => HephaestusHostServiceState::Unknown,
    }
}
