// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Agent-to-controller report types for Hephaestus-managed nodes.

use std::{fmt, net::IpAddr};

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    DockerContainerName, DockerImageReference, HephaestusPrivateIpAddress,
    HephaestusProviderServerId, HephaestusServerApp,
};

use super::HephaestusDomainError;

const MAX_LOG_LINE_BYTES: usize = 512;
const MAX_LOG_LINES_PER_CONTAINER: usize = 32;
const MAX_CONTAINERS_PER_REPORT: usize = 128;
const MAX_LOG_LINES_PER_HOST_SERVICE: usize = 32;
const MAX_HOST_SERVICES_PER_REPORT: usize = 128;
const MAX_APPS_PER_REPORT: usize = 128;
const MAX_HOST_SERVICE_UNIT_NAME_BYTES: usize = 128;
const MAX_TAILSCALE_IPS_PER_REPORT: usize = 8;
const MAX_TAILSCALE_TAGS_PER_REPORT: usize = 32;
const MAX_TAILSCALE_SERVICES_PER_REPORT: usize = 32;
const MAX_TAILSCALE_IDENTITY_BYTES: usize = 253;
const MAX_RUNTIME_VERSION_BYTES: usize = 128;
const MAX_SERVICE_PROBES_PER_REPORT: usize = 64;
const MAX_PROBE_NAME_BYTES: usize = 64;
const MAX_PROBE_TARGET_BYTES: usize = 256;

/// Stable container lifecycle state seen by the local client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusContainerState {
    /// Docker has created the container but it is not yet running.
    Created,
    /// The container is currently running.
    Running,
    /// The container is restarting after failure or operator action.
    Restarting,
    /// Docker is removing the container.
    Removing,
    /// The container is paused.
    Paused,
    /// The container has exited.
    Exited,
    /// The container is in a dead terminal state.
    Dead,
    /// The runtime could not map the observed state.
    Unknown,
}

/// Stable container health state seen by the local client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusContainerHealthState {
    /// Container health checks currently pass.
    Healthy,
    /// Container health checks currently fail.
    Unhealthy,
    /// Container health checks are still warming up.
    Starting,
    /// No container health check is configured.
    None,
    /// The runtime could not map the observed health state.
    Unknown,
}

/// Stable reason the local client recommends restarting a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusContainerRestartReason {
    /// No restart is currently recommended.
    None,
    /// Container health checks are failing.
    Unhealthy,
    /// Docker reports the container is currently restarting.
    Restarting,
    /// Docker reports the container has exited.
    Exited,
    /// Docker reports the container is dead.
    Dead,
    /// The runtime could not map the observed container state safely.
    Unknown,
}

/// Stable host-installed service lifecycle state seen by the local client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusHostServiceState {
    /// The service is active and running.
    Active,
    /// The service is currently reloading.
    Reloading,
    /// The service is inactive and not running.
    Inactive,
    /// The service has failed.
    Failed,
    /// The service is activating.
    Activating,
    /// The service is deactivating.
    Deactivating,
    /// The runtime could not map the observed state.
    Unknown,
}

/// Validated systemd unit or host-service identifier.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct HephaestusHostServiceUnitName(String);

impl HephaestusHostServiceUnitName {
    /// Constructs a validated host service unit name.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_HOST_SERVICE_UNIT_NAME_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_host_service_unit_name_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated host service unit name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for HephaestusHostServiceUnitName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusHostServiceUnitName")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for HephaestusHostServiceUnitName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// One bounded log line captured by the local client.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct HephaestusAgentReportLine(String);

impl HephaestusAgentReportLine {
    /// Constructs a validated log line.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_LOG_LINE_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_log_line_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the bounded log line.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for HephaestusAgentReportLine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusAgentReportLine")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for HephaestusAgentReportLine {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Host resource snapshot reported by the local client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusHostResourceReport {
    cpu_usage_basis_points: u16,
    memory_total_bytes: u64,
    memory_used_bytes: u64,
    disk_total_bytes: u64,
    disk_used_bytes: u64,
}

/// Stable unattended-upgrades state observed by the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusUnattendedUpgradesState {
    /// unattended-upgrades is installed and active.
    Active,
    /// unattended-upgrades is installed but inactive.
    Inactive,
    /// unattended-upgrades has failed.
    Failed,
    /// unattended-upgrades is absent on this host.
    NotInstalled,
    /// The agent could not determine state.
    Unknown,
}

/// Stable service probe kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusServiceProbeKind {
    /// HTTP GET probe.
    Http,
    /// TCP connect probe.
    Tcp,
}

/// Stable service probe result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusServiceProbeStatus {
    /// Probe succeeded.
    Ok,
    /// Probe failed.
    Failed,
    /// Probe was configured but intentionally skipped.
    Skipped,
}

/// Node-local Tailscale identity observed by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusTailscaleReport {
    hostname: Option<String>,
    magic_dns_name: Option<String>,
    ips: Vec<String>,
    tags: Vec<String>,
    services: Vec<String>,
}

impl HephaestusTailscaleReport {
    /// Constructs a bounded Tailscale identity report.
    pub fn new(
        hostname: Option<String>,
        magic_dns_name: Option<String>,
        ips: Vec<String>,
        tags: Vec<String>,
        services: Vec<String>,
    ) -> Result<Self, HephaestusDomainError> {
        if ips.len() > MAX_TAILSCALE_IPS_PER_REPORT
            || tags.len() > MAX_TAILSCALE_TAGS_PER_REPORT
            || services.len() > MAX_TAILSCALE_SERVICES_PER_REPORT
        {
            return Err(HephaestusDomainError::TooLong);
        }
        if let Some(value) = &hostname {
            validate_tailnet_identity(value)?;
        }
        if let Some(value) = &magic_dns_name {
            validate_tailnet_identity(value)?;
        }
        for ip in &ips {
            validate_ip_literal(ip)?;
        }
        for tag in &tags {
            validate_tailnet_identity(tag)?;
        }
        for service in &services {
            validate_tailnet_identity(service)?;
        }
        Ok(Self {
            hostname,
            magic_dns_name,
            ips,
            tags,
            services,
        })
    }

    /// Returns an empty Tailscale report for hosts that have not joined yet.
    pub fn empty() -> Self {
        Self {
            hostname: None,
            magic_dns_name: None,
            ips: Vec::new(),
            tags: Vec::new(),
            services: Vec::new(),
        }
    }

    /// Returns the observed Tailscale hostname.
    pub fn hostname(&self) -> Option<&str> {
        self.hostname.as_deref()
    }

    /// Returns the observed MagicDNS name.
    pub fn magic_dns_name(&self) -> Option<&str> {
        self.magic_dns_name.as_deref()
    }

    /// Returns observed Tailscale IPs.
    pub fn ips(&self) -> &[String] {
        self.ips.as_slice()
    }

    /// Returns observed Tailscale tags.
    pub fn tags(&self) -> &[String] {
        self.tags.as_slice()
    }

    /// Returns observed advertised Tailscale Services.
    pub fn services(&self) -> &[String] {
        self.services.as_slice()
    }
}

/// Docker and Compose versions observed by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusRuntimeVersionReport {
    docker_version: Option<String>,
    docker_compose_version: Option<String>,
}

impl HephaestusRuntimeVersionReport {
    /// Constructs a bounded runtime version report.
    pub fn new(
        docker_version: Option<String>,
        docker_compose_version: Option<String>,
    ) -> Result<Self, HephaestusDomainError> {
        if let Some(value) = &docker_version {
            validate_runtime_version(value)?;
        }
        if let Some(value) = &docker_compose_version {
            validate_runtime_version(value)?;
        }
        Ok(Self {
            docker_version,
            docker_compose_version,
        })
    }

    /// Returns an empty version report.
    pub const fn empty() -> Self {
        Self {
            docker_version: None,
            docker_compose_version: None,
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

/// Host patch/reboot posture observed by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusPatchStateReport {
    reboot_required: bool,
    unattended_upgrades_state: HephaestusUnattendedUpgradesState,
}

impl HephaestusPatchStateReport {
    /// Constructs a patch-state report.
    pub const fn new(
        reboot_required: bool,
        unattended_upgrades_state: HephaestusUnattendedUpgradesState,
    ) -> Self {
        Self {
            reboot_required,
            unattended_upgrades_state,
        }
    }

    /// Returns an unknown patch-state report.
    pub const fn unknown() -> Self {
        Self {
            reboot_required: false,
            unattended_upgrades_state: HephaestusUnattendedUpgradesState::Unknown,
        }
    }

    /// Returns whether a reboot is required.
    pub const fn reboot_required(&self) -> bool {
        self.reboot_required
    }

    /// Returns unattended-upgrades health.
    pub const fn unattended_upgrades_state(&self) -> HephaestusUnattendedUpgradesState {
        self.unattended_upgrades_state
    }
}

/// Bounded cAdvisor metrics summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusCadvisorSummary {
    reachable: bool,
    container_metric_lines: u32,
    has_cpu_metrics: bool,
    has_memory_metrics: bool,
}

impl HephaestusCadvisorSummary {
    /// Constructs a cAdvisor summary.
    pub const fn new(
        reachable: bool,
        container_metric_lines: u32,
        has_cpu_metrics: bool,
        has_memory_metrics: bool,
    ) -> Self {
        Self {
            reachable,
            container_metric_lines,
            has_cpu_metrics,
            has_memory_metrics,
        }
    }

    /// Returns an unreachable cAdvisor summary.
    pub const fn unreachable() -> Self {
        Self::new(false, 0, false, false)
    }

    /// Returns whether cAdvisor was reachable.
    pub const fn reachable(self) -> bool {
        self.reachable
    }

    /// Returns observed container metric lines.
    pub const fn container_metric_lines(self) -> u32 {
        self.container_metric_lines
    }

    /// Returns whether cAdvisor CPU metrics were observed.
    pub const fn has_cpu_metrics(self) -> bool {
        self.has_cpu_metrics
    }

    /// Returns whether cAdvisor memory metrics were observed.
    pub const fn has_memory_metrics(self) -> bool {
        self.has_memory_metrics
    }
}

/// Bounded node-exporter metrics summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusNodeExporterSummary {
    reachable: bool,
    metric_lines: u32,
    has_cpu_metrics: bool,
    has_memory_metrics: bool,
    has_filesystem_metrics: bool,
}

impl HephaestusNodeExporterSummary {
    /// Constructs a node-exporter summary.
    pub const fn new(
        reachable: bool,
        metric_lines: u32,
        has_cpu_metrics: bool,
        has_memory_metrics: bool,
        has_filesystem_metrics: bool,
    ) -> Self {
        Self {
            reachable,
            metric_lines,
            has_cpu_metrics,
            has_memory_metrics,
            has_filesystem_metrics,
        }
    }

    /// Returns an unreachable node-exporter summary.
    pub const fn unreachable() -> Self {
        Self::new(false, 0, false, false, false)
    }

    /// Returns whether node-exporter was reachable.
    pub const fn reachable(self) -> bool {
        self.reachable
    }

    /// Returns observed metric lines.
    pub const fn metric_lines(self) -> u32 {
        self.metric_lines
    }

    /// Returns whether CPU metrics were observed.
    pub const fn has_cpu_metrics(self) -> bool {
        self.has_cpu_metrics
    }

    /// Returns whether memory metrics were observed.
    pub const fn has_memory_metrics(self) -> bool {
        self.has_memory_metrics
    }

    /// Returns whether filesystem metrics were observed.
    pub const fn has_filesystem_metrics(self) -> bool {
        self.has_filesystem_metrics
    }
}

/// Local observability endpoint summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusObservabilityReport {
    cadvisor: HephaestusCadvisorSummary,
    node_exporter: HephaestusNodeExporterSummary,
}

impl HephaestusObservabilityReport {
    /// Constructs an observability report.
    pub const fn new(
        cadvisor: HephaestusCadvisorSummary,
        node_exporter: HephaestusNodeExporterSummary,
    ) -> Self {
        Self {
            cadvisor,
            node_exporter,
        }
    }

    /// Returns an empty observability report.
    pub const fn empty() -> Self {
        Self::new(
            HephaestusCadvisorSummary::unreachable(),
            HephaestusNodeExporterSummary::unreachable(),
        )
    }

    /// Returns the cAdvisor summary.
    pub const fn cadvisor(self) -> HephaestusCadvisorSummary {
        self.cadvisor
    }

    /// Returns the node-exporter summary.
    pub const fn node_exporter(self) -> HephaestusNodeExporterSummary {
        self.node_exporter
    }
}

impl HephaestusHostResourceReport {
    /// Constructs a validated host resource snapshot.
    pub fn new(
        cpu_usage_basis_points: u16,
        memory_total_bytes: u64,
        memory_used_bytes: u64,
        disk_total_bytes: u64,
        disk_used_bytes: u64,
    ) -> Result<Self, HephaestusDomainError> {
        if cpu_usage_basis_points > 10_000
            || memory_used_bytes > memory_total_bytes
            || disk_used_bytes > disk_total_bytes
        {
            return Err(HephaestusDomainError::InvalidNumber);
        }
        Ok(Self {
            cpu_usage_basis_points,
            memory_total_bytes,
            memory_used_bytes,
            disk_total_bytes,
            disk_used_bytes,
        })
    }

    /// Returns CPU usage in basis points, where `10_000` is 100%.
    pub const fn cpu_usage_basis_points(&self) -> u16 {
        self.cpu_usage_basis_points
    }

    /// Returns total host memory in bytes.
    pub const fn memory_total_bytes(&self) -> u64 {
        self.memory_total_bytes
    }

    /// Returns currently used host memory in bytes.
    pub const fn memory_used_bytes(&self) -> u64 {
        self.memory_used_bytes
    }

    /// Returns total disk capacity in bytes for the observed root filesystem.
    pub const fn disk_total_bytes(&self) -> u64 {
        self.disk_total_bytes
    }

    /// Returns used disk capacity in bytes for the observed root filesystem.
    pub const fn disk_used_bytes(&self) -> u64 {
        self.disk_used_bytes
    }
}

/// One container snapshot with bounded recent log lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusContainerReport {
    name: DockerContainerName,
    image: DockerImageReference,
    state: HephaestusContainerState,
    health: HephaestusContainerHealthState,
    recent_logs: Vec<HephaestusAgentReportLine>,
    restart_required: bool,
    restart_reason: HephaestusContainerRestartReason,
}

impl HephaestusContainerReport {
    /// Constructs a container report.
    pub fn new(
        name: DockerContainerName,
        image: DockerImageReference,
        state: HephaestusContainerState,
        health: HephaestusContainerHealthState,
        recent_logs: Vec<HephaestusAgentReportLine>,
        restart_required: bool,
        restart_reason: HephaestusContainerRestartReason,
    ) -> Result<Self, HephaestusDomainError> {
        if recent_logs.len() > MAX_LOG_LINES_PER_CONTAINER {
            return Err(HephaestusDomainError::TooLong);
        }
        if !restart_required && restart_reason != HephaestusContainerRestartReason::None {
            return Err(HephaestusDomainError::InvalidNumber);
        }
        Ok(Self {
            name,
            image,
            state,
            health,
            recent_logs,
            restart_required,
            restart_reason,
        })
    }

    /// Returns the container name.
    pub const fn name(&self) -> &DockerContainerName {
        &self.name
    }

    /// Returns the container image reference.
    pub const fn image(&self) -> &DockerImageReference {
        &self.image
    }

    /// Returns the mapped container lifecycle state.
    pub const fn state(&self) -> HephaestusContainerState {
        self.state
    }

    /// Returns the mapped container health state.
    pub const fn health(&self) -> HephaestusContainerHealthState {
        self.health
    }

    /// Returns recent bounded log lines captured for the container.
    pub fn recent_logs(&self) -> &[HephaestusAgentReportLine] {
        self.recent_logs.as_slice()
    }

    /// Returns whether the agent recommends restarting the container.
    pub const fn restart_required(&self) -> bool {
        self.restart_required
    }

    /// Returns the stable restart recommendation reason.
    pub const fn restart_reason(&self) -> HephaestusContainerRestartReason {
        self.restart_reason
    }
}

/// One host-installed service snapshot with bounded recent log lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusHostServiceReport {
    unit_name: HephaestusHostServiceUnitName,
    state: HephaestusHostServiceState,
    recent_logs: Vec<HephaestusAgentReportLine>,
}

impl HephaestusHostServiceReport {
    /// Constructs a host service report.
    pub fn new(
        unit_name: HephaestusHostServiceUnitName,
        state: HephaestusHostServiceState,
        recent_logs: Vec<HephaestusAgentReportLine>,
    ) -> Result<Self, HephaestusDomainError> {
        if recent_logs.len() > MAX_LOG_LINES_PER_HOST_SERVICE {
            return Err(HephaestusDomainError::TooLong);
        }
        Ok(Self {
            unit_name,
            state,
            recent_logs,
        })
    }

    /// Returns the host service unit name.
    pub const fn unit_name(&self) -> &HephaestusHostServiceUnitName {
        &self.unit_name
    }

    /// Returns the mapped host service lifecycle state.
    pub const fn state(&self) -> HephaestusHostServiceState {
        self.state
    }

    /// Returns recent bounded log lines captured for the host service.
    pub fn recent_logs(&self) -> &[HephaestusAgentReportLine] {
        self.recent_logs.as_slice()
    }
}

/// One bounded local service probe result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusServiceProbeReport {
    probe_name: String,
    service_name: String,
    kind: HephaestusServiceProbeKind,
    target: String,
    status: HephaestusServiceProbeStatus,
    http_status_code: Option<u16>,
}

impl HephaestusServiceProbeReport {
    /// Constructs a service probe report.
    pub fn new(
        probe_name: String,
        service_name: String,
        kind: HephaestusServiceProbeKind,
        target: String,
        status: HephaestusServiceProbeStatus,
        http_status_code: Option<u16>,
    ) -> Result<Self, HephaestusDomainError> {
        validate_probe_identity(probe_name.as_str())?;
        validate_probe_identity(service_name.as_str())?;
        validate_probe_target(target.as_str())?;
        if matches!(kind, HephaestusServiceProbeKind::Tcp) && http_status_code.is_some() {
            return Err(HephaestusDomainError::InvalidNumber);
        }
        Ok(Self {
            probe_name,
            service_name,
            kind,
            target,
            status,
            http_status_code,
        })
    }

    /// Returns the stable probe name.
    pub fn probe_name(&self) -> &str {
        self.probe_name.as_str()
    }

    /// Returns the service name.
    pub fn service_name(&self) -> &str {
        self.service_name.as_str()
    }

    /// Returns the probe kind.
    pub const fn kind(&self) -> HephaestusServiceProbeKind {
        self.kind
    }

    /// Returns the non-secret probe target.
    pub fn target(&self) -> &str {
        self.target.as_str()
    }

    /// Returns the probe status.
    pub const fn status(&self) -> HephaestusServiceProbeStatus {
        self.status
    }

    /// Returns the HTTP status code for HTTP probes.
    pub const fn http_status_code(&self) -> Option<u16> {
        self.http_status_code
    }
}

/// Complete bounded observation of node-local workloads and services.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusObservedServiceSet {
    containers: Vec<HephaestusContainerReport>,
    host_services: Vec<HephaestusHostServiceReport>,
    actual_apps: Vec<HephaestusServerApp>,
}

impl HephaestusObservedServiceSet {
    /// Constructs a validated service observation set.
    pub fn new(
        containers: Vec<HephaestusContainerReport>,
        host_services: Vec<HephaestusHostServiceReport>,
        actual_apps: Vec<HephaestusServerApp>,
    ) -> Result<Self, HephaestusDomainError> {
        if containers.len() > MAX_CONTAINERS_PER_REPORT {
            return Err(HephaestusDomainError::TooLong);
        }
        if host_services.len() > MAX_HOST_SERVICES_PER_REPORT {
            return Err(HephaestusDomainError::TooLong);
        }
        if actual_apps.len() > MAX_APPS_PER_REPORT {
            return Err(HephaestusDomainError::TooLong);
        }
        Ok(Self {
            containers,
            host_services,
            actual_apps,
        })
    }

    /// Returns container snapshots captured by the client.
    pub fn containers(&self) -> &[HephaestusContainerReport] {
        self.containers.as_slice()
    }

    /// Returns host-installed service snapshots captured by the client.
    pub fn host_services(&self) -> &[HephaestusHostServiceReport] {
        self.host_services.as_slice()
    }

    /// Returns actual ReallyMe apps discovered by the local client.
    pub fn actual_apps(&self) -> &[HephaestusServerApp] {
        self.actual_apps.as_slice()
    }
}

/// Complete agent report accepted by Hephaestus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusAgentReport {
    server_id: super::HephaestusServerId,
    provider_server_id: HephaestusProviderServerId,
    private_ip: Option<HephaestusPrivateIpAddress>,
    generated_at_unix_secs: u64,
    resources: HephaestusHostResourceReport,
    observed_services: HephaestusObservedServiceSet,
    tailscale: HephaestusTailscaleReport,
    runtime_versions: HephaestusRuntimeVersionReport,
    patch_state: HephaestusPatchStateReport,
    observability: HephaestusObservabilityReport,
    service_probes: Vec<HephaestusServiceProbeReport>,
}

impl HephaestusAgentReport {
    /// Constructs a validated agent report.
    pub fn new(
        server_id: super::HephaestusServerId,
        provider_server_id: HephaestusProviderServerId,
        private_ip: Option<HephaestusPrivateIpAddress>,
        generated_at_unix_secs: u64,
        resources: HephaestusHostResourceReport,
        observed_services: HephaestusObservedServiceSet,
    ) -> Result<Self, HephaestusDomainError> {
        if generated_at_unix_secs == 0 {
            return Err(HephaestusDomainError::InvalidNumber);
        }
        Ok(Self {
            server_id,
            provider_server_id,
            private_ip,
            generated_at_unix_secs,
            resources,
            observed_services,
            tailscale: HephaestusTailscaleReport::empty(),
            runtime_versions: HephaestusRuntimeVersionReport::empty(),
            patch_state: HephaestusPatchStateReport::unknown(),
            observability: HephaestusObservabilityReport::empty(),
            service_probes: Vec::new(),
        })
    }

    /// Adds expanded host-health details collected by the agent.
    pub fn with_host_health_details(
        mut self,
        tailscale: HephaestusTailscaleReport,
        runtime_versions: HephaestusRuntimeVersionReport,
        patch_state: HephaestusPatchStateReport,
        observability: HephaestusObservabilityReport,
        service_probes: Vec<HephaestusServiceProbeReport>,
    ) -> Result<Self, HephaestusDomainError> {
        if service_probes.len() > MAX_SERVICE_PROBES_PER_REPORT {
            return Err(HephaestusDomainError::TooLong);
        }
        self.tailscale = tailscale;
        self.runtime_versions = runtime_versions;
        self.patch_state = patch_state;
        self.observability = observability;
        self.service_probes = service_probes;
        Ok(self)
    }

    /// Returns the internal server identifier.
    pub const fn server_id(&self) -> &super::HephaestusServerId {
        &self.server_id
    }

    /// Returns the provider-backed server identifier.
    pub const fn provider_server_id(&self) -> &HephaestusProviderServerId {
        &self.provider_server_id
    }

    /// Returns the node private IP address when known.
    pub const fn private_ip(&self) -> Option<&HephaestusPrivateIpAddress> {
        self.private_ip.as_ref()
    }

    /// Returns the UNIX timestamp when the report was generated.
    pub const fn generated_at_unix_secs(&self) -> u64 {
        self.generated_at_unix_secs
    }

    /// Returns the host resource snapshot.
    pub const fn resources(&self) -> &HephaestusHostResourceReport {
        &self.resources
    }

    /// Returns container snapshots captured by the client.
    pub fn containers(&self) -> &[HephaestusContainerReport] {
        self.observed_services.containers()
    }

    /// Returns host-installed service snapshots captured by the client.
    pub fn host_services(&self) -> &[HephaestusHostServiceReport] {
        self.observed_services.host_services()
    }

    /// Returns actual ReallyMe apps discovered by the local client.
    pub fn actual_apps(&self) -> &[HephaestusServerApp] {
        self.observed_services.actual_apps()
    }

    /// Returns the Tailscale identity snapshot.
    pub const fn tailscale(&self) -> &HephaestusTailscaleReport {
        &self.tailscale
    }

    /// Returns Docker and Compose runtime versions.
    pub const fn runtime_versions(&self) -> &HephaestusRuntimeVersionReport {
        &self.runtime_versions
    }

    /// Returns patch/reboot posture.
    pub const fn patch_state(&self) -> &HephaestusPatchStateReport {
        &self.patch_state
    }

    /// Returns local observability endpoint summaries.
    pub const fn observability(&self) -> HephaestusObservabilityReport {
        self.observability
    }

    /// Returns service-specific probe reports.
    pub fn service_probes(&self) -> &[HephaestusServiceProbeReport] {
        self.service_probes.as_slice()
    }
}

fn is_log_line_byte(byte: u8) -> bool {
    byte == b'\t' || (byte.is_ascii() && !byte.is_ascii_control())
}

fn is_host_service_unit_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'@' | b':')
}

fn validate_tailnet_identity(value: &str) -> Result<(), HephaestusDomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(HephaestusDomainError::Empty);
    }
    if trimmed.len() > MAX_TAILSCALE_IDENTITY_BYTES {
        return Err(HephaestusDomainError::TooLong);
    }
    if !trimmed
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(HephaestusDomainError::InvalidCharacter);
    }
    Ok(())
}

fn validate_ip_literal(value: &str) -> Result<(), HephaestusDomainError> {
    value
        .parse::<IpAddr>()
        .map(|_ip| ())
        .map_err(|_error| HephaestusDomainError::InvalidCharacter)
}

fn validate_runtime_version(value: &str) -> Result<(), HephaestusDomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(HephaestusDomainError::Empty);
    }
    if trimmed.len() > MAX_RUNTIME_VERSION_BYTES {
        return Err(HephaestusDomainError::TooLong);
    }
    if !trimmed
        .bytes()
        .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    {
        return Err(HephaestusDomainError::InvalidCharacter);
    }
    Ok(())
}

fn validate_probe_identity(value: &str) -> Result<(), HephaestusDomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(HephaestusDomainError::Empty);
    }
    if trimmed.len() > MAX_PROBE_NAME_BYTES {
        return Err(HephaestusDomainError::TooLong);
    }
    if !trimmed
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(HephaestusDomainError::InvalidCharacter);
    }
    Ok(())
}

fn validate_probe_target(value: &str) -> Result<(), HephaestusDomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(HephaestusDomainError::Empty);
    }
    if trimmed.len() > MAX_PROBE_TARGET_BYTES {
        return Err(HephaestusDomainError::TooLong);
    }
    if !trimmed
        .bytes()
        .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    {
        return Err(HephaestusDomainError::InvalidCharacter);
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "fixed test fixtures should fail loudly if validation invariants change"
)]
mod tests {
    use super::{
        HephaestusAgentReport, HephaestusAgentReportLine, HephaestusContainerHealthState,
        HephaestusContainerReport, HephaestusContainerRestartReason, HephaestusContainerState,
        HephaestusHostResourceReport, HephaestusHostServiceReport, HephaestusHostServiceState,
        HephaestusHostServiceUnitName, HephaestusObservedServiceSet,
    };
    use crate::{
        DockerContainerName, DockerImageReference, HephaestusPrivateIpAddress,
        HephaestusProviderServerId, HephaestusServerApp, HephaestusServerId,
    };

    #[test]
    fn host_resource_report_rejects_invalid_usage_bounds() {
        assert!(HephaestusHostResourceReport::new(10_001, 10, 9, 10, 9).is_err());
        assert!(HephaestusHostResourceReport::new(500, 10, 11, 10, 9).is_err());
    }

    #[test]
    fn agent_report_accepts_bounded_container_snapshot() {
        let container = HephaestusContainerReport::new(
            DockerContainerName::new("app").expect("valid name"),
            DockerImageReference::new("ghcr.io/reallyme/app:latest").expect("valid image"),
            HephaestusContainerState::Running,
            HephaestusContainerHealthState::Healthy,
            vec![HephaestusAgentReportLine::new("service ready").expect("valid log line")],
            false,
            HephaestusContainerRestartReason::None,
        )
        .expect("valid container report");
        let resources =
            HephaestusHostResourceReport::new(2500, 1024, 512, 2048, 1024).expect("valid usage");
        let host_service = HephaestusHostServiceReport::new(
            HephaestusHostServiceUnitName::new("foundationdb.service").expect("valid unit"),
            HephaestusHostServiceState::Active,
            vec![HephaestusAgentReportLine::new("cluster available").expect("valid log line")],
        )
        .expect("valid host service report");

        let report = HephaestusAgentReport::new(
            HephaestusServerId::new("srv-lhr-01").expect("valid server id"),
            HephaestusProviderServerId::new("provider-server-01")
                .expect("valid provider server id"),
            Some(HephaestusPrivateIpAddress::new("10.1.1.12").expect("valid private ip")),
            1_715_726_400,
            resources,
            HephaestusObservedServiceSet::new(
                vec![container],
                vec![host_service],
                vec![HephaestusServerApp::new("reallyme-api").expect("valid app")],
            )
            .expect("valid observed service set"),
        );

        assert!(report.is_ok());
    }
}
