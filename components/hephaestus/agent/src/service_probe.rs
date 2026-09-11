// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Local service probe execution.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use reallyme_hephaestus_domain::{
    HephaestusServiceProbeKind, HephaestusServiceProbeReport, HephaestusServiceProbeStatus,
};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::config::{ServiceProbeConfig, ServiceProbeKind};
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};
use crate::http_client::shared_http_client;

/// Runs configured local service probes.
pub async fn collect_service_probes(
    probes: &[ServiceProbeConfig],
    command_timeout: Duration,
) -> AgentResult<Vec<HephaestusServiceProbeReport>> {
    let mut reports = Vec::with_capacity(probes.len());
    for probe in probes {
        reports.push(run_probe(probe, command_timeout).await?);
    }
    Ok(reports)
}

async fn run_probe(
    probe: &ServiceProbeConfig,
    command_timeout: Duration,
) -> AgentResult<HephaestusServiceProbeReport> {
    match probe.kind() {
        ServiceProbeKind::Http => run_http_probe(probe, command_timeout).await,
        ServiceProbeKind::Tcp => run_tcp_probe(probe, command_timeout).await,
    }
}

async fn run_http_probe(
    probe: &ServiceProbeConfig,
    command_timeout: Duration,
) -> AgentResult<HephaestusServiceProbeReport> {
    let response = run_http_probe_raw(probe.target(), command_timeout).await?;
    let (status, http_status_code) = match response {
        Some(status_code) => {
            let status = if (200..=299).contains(&status_code) {
                HephaestusServiceProbeStatus::Ok
            } else {
                HephaestusServiceProbeStatus::Failed
            };
            (status, Some(status_code))
        }
        None => (HephaestusServiceProbeStatus::Failed, None),
    };
    HephaestusServiceProbeReport::new(
        probe.name().to_owned(),
        probe.service_name().to_owned(),
        HephaestusServiceProbeKind::Http,
        probe.target().to_owned(),
        status,
        http_status_code,
    )
    .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::DomainValidationFailed))
}

async fn run_tcp_probe(
    probe: &ServiceProbeConfig,
    command_timeout: Duration,
) -> AgentResult<HephaestusServiceProbeReport> {
    let status = if run_ad_hoc_tcp_probe(probe.target(), command_timeout).await? {
        HephaestusServiceProbeStatus::Ok
    } else {
        HephaestusServiceProbeStatus::Failed
    };
    HephaestusServiceProbeReport::new(
        probe.name().to_owned(),
        probe.service_name().to_owned(),
        HephaestusServiceProbeKind::Tcp,
        probe.target().to_owned(),
        status,
        None,
    )
    .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::DomainValidationFailed))
}

/// Runs one ad-hoc local HTTP health probe.
pub async fn run_ad_hoc_http_probe(
    target: &str,
    expected_status: u16,
    command_timeout: Duration,
) -> AgentResult<bool> {
    let Some(status_code) = run_http_probe_raw(target, command_timeout).await? else {
        return Ok(false);
    };
    Ok(status_code == expected_status)
}

/// Runs one ad-hoc local TCP probe.
pub async fn run_ad_hoc_tcp_probe(target: &str, command_timeout: Duration) -> AgentResult<bool> {
    let address = parse_local_socket_addr(target)?;
    let status = match timeout(command_timeout, TcpStream::connect(address)).await {
        Ok(Ok(_stream)) => true,
        Ok(Err(_error)) => false,
        Err(_error) => false,
    };
    Ok(status)
}

async fn run_http_probe_raw(target: &str, command_timeout: Duration) -> AgentResult<Option<u16>> {
    match shared_http_client()?
        .get(target)
        .timeout(command_timeout)
        .send()
        .await
    {
        Ok(response) => Ok(Some(response.status().as_u16())),
        Err(_error) => Ok(None),
    }
}

fn parse_local_socket_addr(value: &str) -> AgentResult<SocketAddr> {
    let Some((host, port)) = value.rsplit_once(':') else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ));
    };
    // Keep only numeric loopback literals to avoid host-name indirection.
    // This matches config validation (`127.0.0.1`/`::1` only) and blocks `localhost`.
    let port = port
        .parse::<u16>()
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidConfig))?;
    match host {
        "127.0.0.1" => Ok(SocketAddr::from((Ipv4Addr::LOCALHOST, port))),
        "::1" => Ok(SocketAddr::from((Ipv6Addr::LOCALHOST, port))),
        _ => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use super::parse_local_socket_addr;

    #[test]
    fn parse_local_socket_addr_rejects_localhost() {
        assert!(parse_local_socket_addr("localhost:5432").is_err());
    }

    #[test]
    fn parse_local_socket_addr_accepts_ipv4_loopback() {
        let address = parse_local_socket_addr("127.0.0.1:5432").expect("loopback parse");

        assert_eq!(address.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(address.port(), 5432);
    }

    #[test]
    fn parse_local_socket_addr_accepts_ipv6_loopback() {
        let address = parse_local_socket_addr("::1:5432").expect("ipv6 loopback parse");

        assert_eq!(address.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
        assert_eq!(address.port(), 5432);
    }
}
