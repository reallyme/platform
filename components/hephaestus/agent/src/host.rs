// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host resource observation through `/proc` and fixed system commands.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reallyme_hephaestus_domain::{
    HephaestusDomainError, HephaestusHostResourceReport, HephaestusPatchStateReport,
    HephaestusUnattendedUpgradesState,
};
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

/// Returns current UNIX seconds.
pub fn unix_seconds_now() -> AgentResult<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))
        .map(|duration| duration.as_secs())
}

/// Collects host CPU, memory, and root filesystem usage.
pub async fn collect_host_resources(
    command_timeout: Duration,
) -> AgentResult<HephaestusHostResourceReport> {
    let cpu = collect_cpu_usage_basis_points().await?;
    let (memory_total, memory_used) = read_memory_usage().await?;
    let (disk_total, disk_used) = read_disk_usage(command_timeout).await?;
    HephaestusHostResourceReport::new(cpu, memory_total, memory_used, disk_total, disk_used)
        .map_err(map_domain_error)
}

/// Returns observed boot time in UNIX seconds.
pub async fn collect_boot_time_unix_secs() -> AgentResult<u64> {
    let contents = tokio::fs::read_to_string("/proc/stat")
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed))?;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("btime ") {
            return value.trim().parse::<u64>().map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
            });
        }
    }
    Err(HephaestusAgentError::new(
        HephaestusAgentErrorReason::CommandOutputInvalid,
    ))
}

/// Collects reboot-required and unattended-upgrades posture.
pub async fn collect_patch_state(
    command_timeout: Duration,
) -> AgentResult<HephaestusPatchStateReport> {
    let reboot_required = tokio::fs::metadata("/var/run/reboot-required")
        .await
        .map(|_metadata| true)
        .unwrap_or(false);
    let unattended_upgrades_state = collect_unattended_upgrades_state(command_timeout).await?;
    Ok(HephaestusPatchStateReport::new(
        reboot_required,
        unattended_upgrades_state,
    ))
}

async fn collect_unattended_upgrades_state(
    command_timeout: Duration,
) -> AgentResult<HephaestusUnattendedUpgradesState> {
    let mut command = Command::new("/bin/systemctl");
    command.args(["is-active", "unattended-upgrades.service"]);
    let output = timeout(command_timeout, crate::command::output(&mut command))
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    let stdout = String::from_utf8(output.stdout).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })?;
    let state = stdout.lines().next().map(str::trim).unwrap_or_default();
    let mapped = match state {
        "active" => HephaestusUnattendedUpgradesState::Active,
        "inactive" => HephaestusUnattendedUpgradesState::Inactive,
        "failed" => HephaestusUnattendedUpgradesState::Failed,
        "unknown" => HephaestusUnattendedUpgradesState::NotInstalled,
        _ => {
            if output.status.success() {
                HephaestusUnattendedUpgradesState::Unknown
            } else {
                HephaestusUnattendedUpgradesState::NotInstalled
            }
        }
    };
    Ok(mapped)
}

async fn collect_cpu_usage_basis_points() -> AgentResult<u16> {
    let first = read_cpu_sample().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let second = read_cpu_sample().await?;
    let total_delta = second.total.saturating_sub(first.total);
    let idle_delta = second.idle.saturating_sub(first.idle);
    if total_delta == 0 {
        return Ok(0);
    }
    let used = total_delta.saturating_sub(idle_delta);
    let basis_points = used
        .checked_mul(10_000)
        .and_then(|value| value.checked_div(total_delta))
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
    u16::try_from(basis_points)
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))
}

#[derive(Debug, Clone, Copy)]
struct CpuSample {
    total: u64,
    idle: u64,
}

async fn read_cpu_sample() -> AgentResult<CpuSample> {
    let contents = tokio::fs::read_to_string("/proc/stat")
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed))?;
    let Some(line) = contents.lines().find(|line| line.starts_with("cpu ")) else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandOutputInvalid,
        ));
    };
    let values = line
        .split_whitespace()
        .skip(1)
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        })?;
    if values.len() < 5 {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandOutputInvalid,
        ));
    }
    let total = values
        .iter()
        .try_fold(0u64, |acc, value| acc.checked_add(*value));
    let Some(total) = total else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidNumber,
        ));
    };
    let idle = values[3]
        .checked_add(values[4])
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
    Ok(CpuSample { total, idle })
}

async fn read_memory_usage() -> AgentResult<(u64, u64)> {
    let contents = tokio::fs::read_to_string("/proc/meminfo")
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed))?;
    let total = meminfo_kib(&contents, "MemTotal:")?;
    let available = meminfo_kib(&contents, "MemAvailable:")?;
    let total_bytes = total
        .checked_mul(1024)
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
    let available_bytes = available
        .checked_mul(1024)
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
    let used = total_bytes.saturating_sub(available_bytes);
    Ok((total_bytes, used))
}

fn meminfo_kib(contents: &str, key: &str) -> AgentResult<u64> {
    for line in contents.lines() {
        if line.starts_with(key) {
            let Some(value) = line.split_whitespace().nth(1) else {
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::CommandOutputInvalid,
                ));
            };
            return value.parse::<u64>().map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
            });
        }
    }
    Err(HephaestusAgentError::new(
        HephaestusAgentErrorReason::CommandOutputInvalid,
    ))
}

async fn read_disk_usage(command_timeout: Duration) -> AgentResult<(u64, u64)> {
    let mut command = Command::new("/bin/df");
    command.args(["-B1", "--output=size,used", "/"]);
    let output = timeout(command_timeout, crate::command::output(&mut command))
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    if !output.status.success() {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        ));
    }
    let stdout = String::from_utf8(output.stdout).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })?;
    let Some(line) = stdout.lines().nth(1) else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandOutputInvalid,
        ));
    };
    let mut parts = line.split_whitespace();
    let total = parts
        .next()
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid))?
        .parse::<u64>()
        .map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        })?;
    let used = parts
        .next()
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid))?
        .parse::<u64>()
        .map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        })?;
    Ok((total, used))
}

fn map_domain_error(_error: HephaestusDomainError) -> HephaestusAgentError {
    HephaestusAgentError::new(HephaestusAgentErrorReason::DomainValidationFailed)
}
