// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Local report assembly.

use reallyme_hephaestus_domain::{
    HephaestusAgentBootReport, HephaestusAgentReport, HephaestusCadvisorSummary,
    HephaestusHostResourceReport, HephaestusNodeExporterSummary, HephaestusObservabilityReport,
    HephaestusObservedServiceSet, HephaestusPrivateIpAddress, HephaestusProviderServerId,
    HephaestusPublicIpAddress, HephaestusRuntimeVersionReport, HephaestusServerApp,
    HephaestusServerBootTiming, HephaestusServerId, HephaestusServerIdentity,
    HephaestusServerNetwork, HephaestusSiteId, HephaestusTailscaleReport,
};

use crate::cadvisor::collect_cadvisor_metrics;
use crate::config::HephaestusAgentConfig;
use crate::docker::{collect_containers, collect_runtime_versions};
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};
use crate::host::{
    collect_boot_time_unix_secs, collect_host_resources, collect_patch_state, unix_seconds_now,
};
use crate::node_exporter::collect_node_exporter_metrics;
use crate::service_probe::collect_service_probes;
use crate::systemd::collect_systemd_units;
use crate::tailscale::{TailscaleSnapshot, collect_tailscale_snapshot};

/// Builds one boot report from local identity and observed network state.
pub async fn build_boot_report(
    config: &HephaestusAgentConfig,
) -> AgentResult<HephaestusAgentBootReport> {
    let generated_at = unix_seconds_now()?;
    let boot_time = collect_boot_time_unix_secs().await?;
    let tailnet = collect_tailscale_snapshot(config.command_timeout()).await?;
    HephaestusAgentBootReport::new(
        HephaestusServerIdentity::new(
            HephaestusServerId::new(config.server_id()).map_err(|_error| domain_error())?,
            HephaestusProviderServerId::new(config.provider_server_id())
                .map_err(|_error| domain_error())?,
            HephaestusSiteId::new(config.site_id()).map_err(|_error| domain_error())?,
        ),
        HephaestusServerNetwork::new(
            None::<HephaestusPublicIpAddress>,
            private_ip(tailnet.ipv4())?,
        ),
        HephaestusServerBootTiming::new(boot_time, generated_at)
            .map_err(|_error| domain_error())?,
    )
    .map_err(|_error| domain_error())
}

/// Builds one heartbeat report from Docker, systemd, cAdvisor, and host state.
pub async fn build_agent_report(
    config: &HephaestusAgentConfig,
) -> AgentResult<HephaestusAgentReport> {
    let generated_at = unix_seconds_now()?;
    let resources = match collect_host_resources(config.command_timeout()).await {
        Ok(resources) => resources,
        Err(_error) => fallback_host_resources()?,
    };
    let tailnet = match collect_tailscale_snapshot(config.command_timeout()).await {
        Ok(tailnet) => tailnet,
        Err(_error) => fallback_tailscale_snapshot()?,
    };
    let containers = collect_containers(config.command_timeout())
        .await
        .unwrap_or_default();
    let runtime_versions = collect_runtime_versions(config.command_timeout())
        .await
        .ok()
        .and_then(|versions| {
            HephaestusRuntimeVersionReport::new(
                versions.docker_version().map(str::to_owned),
                versions.docker_compose_version().map(str::to_owned),
            )
            .ok()
        })
        .unwrap_or_else(HephaestusRuntimeVersionReport::empty);
    let host_services =
        collect_systemd_units(config.observed_systemd_units(), config.command_timeout())
            .await
            .unwrap_or_default();
    let cadvisor =
        collect_cadvisor_metrics(config.cadvisor_metrics_url(), config.command_timeout())
            .await
            .map(|snapshot| {
                HephaestusCadvisorSummary::new(
                    true,
                    snapshot.container_metric_lines(),
                    snapshot.has_cpu_metrics(),
                    snapshot.has_memory_metrics(),
                )
            })
            .unwrap_or_else(|_error| HephaestusCadvisorSummary::unreachable());
    let node_exporter =
        collect_node_exporter_metrics(config.node_exporter_metrics_url(), config.command_timeout())
            .await
            .map(|snapshot| {
                HephaestusNodeExporterSummary::new(
                    true,
                    snapshot.metric_lines(),
                    snapshot.has_cpu_metrics(),
                    snapshot.has_memory_metrics(),
                    snapshot.has_filesystem_metrics(),
                )
            })
            .unwrap_or_else(|_error| HephaestusNodeExporterSummary::unreachable());
    let observability = HephaestusObservabilityReport::new(cadvisor, node_exporter);
    let patch_state = collect_patch_state(config.command_timeout())
        .await
        .unwrap_or_else(|_error| reallyme_hephaestus_domain::HephaestusPatchStateReport::unknown());
    let service_probes = collect_service_probes(config.service_probes(), config.command_timeout())
        .await
        .unwrap_or_default();
    let actual_apps = config
        .observed_apps()
        .iter()
        .map(|value| HephaestusServerApp::new(value.as_str()).map_err(|_error| domain_error()))
        .collect::<AgentResult<Vec<_>>>()?;
    let observed_services =
        HephaestusObservedServiceSet::new(containers, host_services, actual_apps)
            .map_err(|_error| domain_error())?;

    HephaestusAgentReport::new(
        HephaestusServerId::new(config.server_id()).map_err(|_error| domain_error())?,
        HephaestusProviderServerId::new(config.provider_server_id())
            .map_err(|_error| domain_error())?,
        private_ip(tailnet.ipv4())?,
        generated_at,
        resources,
        observed_services,
    )
    .and_then(|report| {
        report.with_host_health_details(
            tailscale_report(&tailnet)?,
            runtime_versions,
            patch_state,
            observability,
            service_probes,
        )
    })
    .map_err(|_error| domain_error())
}

fn fallback_host_resources() -> AgentResult<HephaestusHostResourceReport> {
    HephaestusHostResourceReport::new(0, 0, 0, 0, 0).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::DomainValidationFailed)
    })
}

fn fallback_tailscale_snapshot() -> AgentResult<TailscaleSnapshot> {
    TailscaleSnapshot::new(None, None, None, Vec::new(), Vec::new(), Vec::new())
}

fn private_ip(value: Option<&str>) -> AgentResult<Option<HephaestusPrivateIpAddress>> {
    value
        .map(HephaestusPrivateIpAddress::new)
        .transpose()
        .map_err(|_error| domain_error())
}

fn tailscale_report(
    value: &TailscaleSnapshot,
) -> Result<HephaestusTailscaleReport, reallyme_hephaestus_domain::HephaestusDomainError> {
    HephaestusTailscaleReport::new(
        value.hostname().map(str::to_owned),
        value.magic_dns_name().map(str::to_owned),
        value.ips().to_vec(),
        value.tags().to_vec(),
        value.services().to_vec(),
    )
}

fn domain_error() -> HephaestusAgentError {
    HephaestusAgentError::new(HephaestusAgentErrorReason::DomainValidationFailed)
}
