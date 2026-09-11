// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validated agent configuration loaded from cloud-init rendered TOML.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use secrecy::SecretString;
use serde::Deserialize;
use url::Url;

use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

const DEFAULT_FULL_REPORT_INTERVAL_SECONDS: u64 = 30;
const MAX_FULL_REPORT_INTERVAL_SECONDS: u64 = 24 * 60 * 60;
const DEFAULT_ACTION_POLL_INTERVAL_SECONDS: u64 = 2;
const MAX_ACTION_POLL_INTERVAL_SECONDS: u64 = 60 * 60;
const DEFAULT_COMMAND_TIMEOUT_SECONDS: u64 = 10;
const MAX_COMMAND_TIMEOUT_SECONDS: u64 = 60 * 60;
const DEFAULT_CADVISOR_METRICS_URL: &str = "http://127.0.0.1:8080/metrics";
const DEFAULT_NODE_EXPORTER_METRICS_URL: &str = "http://127.0.0.1:9100/metrics";
const DEFAULT_STATE_DIR: &str = "/var/lib/reallyme/hephaestus-agent";
const DEFAULT_BOOTSTRAP_TOKEN_PATH: &str = "/etc/reallyme/hephaestus-agent/bootstrap-token";
const DEFAULT_AUDIT_LOG_FILE_NAME: &str = "action-audit.jsonl";
const MAX_CONFIG_BYTES: u64 = 64 * 1024;

/// Agent runtime configuration.
#[derive(Clone)]
pub struct HephaestusAgentConfig {
    controller_base_url: Url,
    server_id: String,
    provider_server_id: String,
    site_id: String,
    bootstrap_token_path: PathBuf,
    inline_bootstrap_token: Option<SecretString>,
    full_report_interval: Duration,
    action_poll_interval: Duration,
    command_timeout: Duration,
    state_dir: PathBuf,
    audit_log_path: PathBuf,
    cadvisor_metrics_url: Url,
    node_exporter_metrics_url: Url,
    observed_systemd_units: Vec<String>,
    observed_apps: Vec<String>,
    service_probes: Vec<ServiceProbeConfig>,
}

/// Configured local service probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceProbeConfig {
    name: String,
    service_name: String,
    kind: ServiceProbeKind,
    target: String,
}

impl ServiceProbeConfig {
    /// Returns the stable probe name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns the service name.
    pub fn service_name(&self) -> &str {
        self.service_name.as_str()
    }

    /// Returns the probe kind.
    pub const fn kind(&self) -> ServiceProbeKind {
        self.kind
    }

    /// Returns the target.
    pub fn target(&self) -> &str {
        self.target.as_str()
    }
}

/// Supported service probe transports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceProbeKind {
    /// HTTP GET probe against a local URL.
    Http,
    /// TCP connect probe against a local host:port.
    Tcp,
}

impl HephaestusAgentConfig {
    /// Loads and validates configuration from a TOML file.
    pub async fn load(path: &Path) -> AgentResult<Self> {
        let metadata = tokio::fs::metadata(path).await.map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed)
        })?;
        if metadata.len() > MAX_CONFIG_BYTES {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::InvalidConfig,
            ));
        }
        let bytes = tokio::fs::read(path).await.map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed)
        })?;
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidConfig)
        })?;
        let raw: RawConfig = toml::from_str(text).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidConfig)
        })?;
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawConfig) -> AgentResult<Self> {
        let controller_base_url = validate_controller_url(raw.controller_base_url.as_str())?;
        let cadvisor_metrics_url = validate_local_http_url(
            raw.cadvisor_metrics_url
                .unwrap_or_else(|| DEFAULT_CADVISOR_METRICS_URL.to_owned())
                .as_str(),
        )?;
        let node_exporter_metrics_url = validate_local_http_url(
            raw.node_exporter_metrics_url
                .unwrap_or_else(|| DEFAULT_NODE_EXPORTER_METRICS_URL.to_owned())
                .as_str(),
        )?;
        let full_report_interval = seconds_or_default(
            raw.full_report_interval_seconds,
            DEFAULT_FULL_REPORT_INTERVAL_SECONDS,
            MAX_FULL_REPORT_INTERVAL_SECONDS,
        )?;
        let action_poll_interval = seconds_or_default(
            raw.action_poll_interval_seconds,
            DEFAULT_ACTION_POLL_INTERVAL_SECONDS,
            MAX_ACTION_POLL_INTERVAL_SECONDS,
        )?;
        let command_timeout = seconds_or_default(
            raw.command_timeout_seconds,
            DEFAULT_COMMAND_TIMEOUT_SECONDS,
            MAX_COMMAND_TIMEOUT_SECONDS,
        )?;
        let state_dir = validate_absolute_path(
            raw.state_dir
                .unwrap_or_else(|| DEFAULT_STATE_DIR.to_owned())
                .as_str(),
        )?;
        let audit_log_path = match raw.audit_log_path {
            Some(path) => validate_absolute_path(path.as_str())?,
            None => state_dir.join(DEFAULT_AUDIT_LOG_FILE_NAME),
        };
        let bootstrap_token_path = validate_absolute_path(
            raw.bootstrap_token_path
                .unwrap_or_else(|| DEFAULT_BOOTSTRAP_TOKEN_PATH.to_owned())
                .as_str(),
        )?;
        validate_identity(raw.server_id.as_str())?;
        validate_identity(raw.provider_server_id.as_str())?;
        validate_identity(raw.site_id.as_str())?;
        for unit in &raw.observed_systemd_units {
            validate_unit(unit.as_str())?;
        }
        for app in &raw.observed_apps {
            validate_identity(app.as_str())?;
        }
        let service_probes = raw
            .service_probes
            .into_iter()
            .map(ServiceProbeConfig::from_raw)
            .collect::<AgentResult<Vec<_>>>()?;
        Ok(Self {
            controller_base_url,
            server_id: raw.server_id,
            provider_server_id: raw.provider_server_id,
            site_id: raw.site_id,
            bootstrap_token_path,
            inline_bootstrap_token: raw
                .boot_secret
                .map(|value| SecretString::new(value.into_boxed_str())),
            full_report_interval,
            action_poll_interval,
            command_timeout,
            state_dir,
            audit_log_path,
            cadvisor_metrics_url,
            node_exporter_metrics_url,
            observed_systemd_units: raw.observed_systemd_units,
            observed_apps: raw.observed_apps,
            service_probes,
        })
    }

    /// Returns the Hephaestus Connect base URL.
    pub const fn controller_base_url(&self) -> &Url {
        &self.controller_base_url
    }

    /// Returns the stable server id.
    pub fn server_id(&self) -> &str {
        self.server_id.as_str()
    }

    /// Returns the provider server id.
    pub fn provider_server_id(&self) -> &str {
        self.provider_server_id.as_str()
    }

    /// Returns the provider-neutral site id.
    pub fn site_id(&self) -> &str {
        self.site_id.as_str()
    }

    /// Returns the cloud-init bootstrap token file path.
    pub fn bootstrap_token_path(&self) -> &Path {
        self.bootstrap_token_path.as_path()
    }

    /// Returns the optional inline bootstrap token for tests and legacy configs.
    pub const fn inline_bootstrap_token(&self) -> Option<&SecretString> {
        self.inline_bootstrap_token.as_ref()
    }

    /// Returns the full telemetry report interval.
    pub const fn full_report_interval(&self) -> Duration {
        self.full_report_interval
    }

    /// Returns the lightweight action poll interval.
    pub const fn action_poll_interval(&self) -> Duration {
        self.action_poll_interval
    }

    /// Returns the timeout for local fixed-argument commands.
    pub const fn command_timeout(&self) -> Duration {
        self.command_timeout
    }

    /// Returns the local state directory.
    pub fn state_dir(&self) -> &Path {
        self.state_dir.as_path()
    }

    /// Returns the append-only local action audit log path.
    pub fn audit_log_path(&self) -> &Path {
        self.audit_log_path.as_path()
    }

    /// Returns the local cAdvisor metrics URL.
    pub const fn cadvisor_metrics_url(&self) -> &Url {
        &self.cadvisor_metrics_url
    }

    /// Returns the local node-exporter metrics URL.
    pub const fn node_exporter_metrics_url(&self) -> &Url {
        &self.node_exporter_metrics_url
    }

    /// Returns configured systemd units to observe.
    pub fn observed_systemd_units(&self) -> &[String] {
        self.observed_systemd_units.as_slice()
    }

    /// Returns configured app ids expected on this host.
    pub fn observed_apps(&self) -> &[String] {
        self.observed_apps.as_slice()
    }

    /// Returns configured local service probes.
    pub fn service_probes(&self) -> &[ServiceProbeConfig] {
        self.service_probes.as_slice()
    }
}

#[derive(Deserialize)]
struct RawConfig {
    controller_base_url: String,
    server_id: String,
    provider_server_id: String,
    site_id: String,
    boot_secret: Option<String>,
    bootstrap_token_path: Option<String>,
    full_report_interval_seconds: Option<u64>,
    action_poll_interval_seconds: Option<u64>,
    command_timeout_seconds: Option<u64>,
    state_dir: Option<String>,
    audit_log_path: Option<String>,
    cadvisor_metrics_url: Option<String>,
    node_exporter_metrics_url: Option<String>,
    #[serde(default)]
    observed_systemd_units: Vec<String>,
    #[serde(default)]
    observed_apps: Vec<String>,
    #[serde(default)]
    service_probes: Vec<RawServiceProbeConfig>,
}

#[derive(Deserialize)]
struct RawServiceProbeConfig {
    name: String,
    service_name: String,
    kind: String,
    target: String,
}

impl ServiceProbeConfig {
    fn from_raw(raw: RawServiceProbeConfig) -> AgentResult<Self> {
        validate_identity(raw.name.as_str())?;
        validate_identity(raw.service_name.as_str())?;
        let kind = match raw.kind.as_str() {
            "http" => {
                validate_local_http_url(raw.target.as_str())?;
                ServiceProbeKind::Http
            }
            "tcp" => {
                validate_local_tcp_target(raw.target.as_str())?;
                ServiceProbeKind::Tcp
            }
            _ => {
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::InvalidConfig,
                ));
            }
        };
        Ok(Self {
            name: raw.name,
            service_name: raw.service_name,
            kind,
            target: raw.target,
        })
    }
}

fn validate_controller_url(value: &str) -> AgentResult<Url> {
    let url = validate_http_or_https_url(value)?;
    if url.scheme() == "http" && !is_private_controller_host(&url) {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    }
    Ok(url)
}

fn validate_local_http_url(value: &str) -> AgentResult<Url> {
    let url = validate_http_or_https_url(value)?;
    let Some(host) = url.host_str() else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    };
    if !is_literal_loopback_host(host) {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    }
    Ok(url)
}

fn validate_http_or_https_url(value: &str) -> AgentResult<Url> {
    let url = Url::parse(value)
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidUrl))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    }
    Ok(url)
}

fn is_private_controller_host(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    if matches!(host, "localhost" | "::1") || host.ends_with(".ts.net") {
        return true;
    }
    if let Ok(ipv4) = host.parse::<Ipv4Addr>() {
        return ipv4.is_loopback() || ipv4.is_private() || is_tailscale_ipv4(ipv4);
    }
    if let Ok(ipv6) = host.parse::<Ipv6Addr>() {
        return ipv6.is_loopback() || is_unique_local_ipv6(ipv6);
    }
    false
}

fn is_literal_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1")
}

fn is_tailscale_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

fn is_unique_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn validate_local_tcp_target(value: &str) -> AgentResult<()> {
    let Some((host, port)) = value.rsplit_once(':') else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    };
    if !is_literal_loopback_host(host) {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    }
    let parsed = port
        .parse::<u16>()
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidUrl))?;
    if parsed == 0 {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    }
    Ok(())
}

fn seconds_or_default(value: Option<u64>, default: u64, maximum: u64) -> AgentResult<Duration> {
    let seconds = value.unwrap_or(default);
    if seconds == 0 || seconds > maximum {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidNumber,
        ));
    }
    Ok(Duration::from_secs(seconds))
}

fn validate_absolute_path(value: &str) -> AgentResult<PathBuf> {
    if !value.starts_with('/') || value.contains('\0') {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ));
    }
    Ok(PathBuf::from(value))
}

fn validate_identity(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    Ok(())
}

fn validate_unit(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'@'))
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
