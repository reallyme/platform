// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tailscale observation and service advertisement helpers.

use std::collections::BTreeMap;
use std::future::Future;
use std::net::IpAddr;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

const TAILSCALE_SERVE_CONFIG_PATH: &str = "/etc/reallyme/tailscale-services/serveconfig.json";
const MAX_SERVICE_STATUS_RECURSION_DEPTH: u8 = 16;

/// Local Tailscale identity observed by the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscaleSnapshot {
    ipv4: Option<String>,
    hostname: Option<String>,
    magic_dns_name: Option<String>,
    ips: Vec<String>,
    tags: Vec<String>,
    services: Vec<String>,
}

impl TailscaleSnapshot {
    /// Constructs a validated Tailscale snapshot.
    pub fn new(
        ipv4: Option<String>,
        hostname: Option<String>,
        magic_dns_name: Option<String>,
        ips: Vec<String>,
        tags: Vec<String>,
        services: Vec<String>,
    ) -> AgentResult<Self> {
        if let Some(value) = &ipv4 {
            validate_ip(value.as_str())?;
        }
        for value in &ips {
            validate_ip(value.as_str())?;
        }
        if let Some(value) = &hostname {
            validate_identity(value.as_str())?;
        }
        if let Some(value) = &magic_dns_name {
            validate_identity(value.as_str())?;
        }
        for value in &tags {
            validate_identity(value.as_str())?;
        }
        for value in &services {
            validate_service_name(value.as_str())?;
        }
        Ok(Self {
            ipv4,
            hostname,
            magic_dns_name,
            ips,
            tags,
            services,
        })
    }

    /// Returns the first Tailscale IPv4 address, if available.
    pub fn ipv4(&self) -> Option<&str> {
        self.ipv4.as_deref()
    }

    /// Returns the local Tailscale hostname, if reported.
    pub fn hostname(&self) -> Option<&str> {
        self.hostname.as_deref()
    }

    /// Returns the MagicDNS name, if reported.
    pub fn magic_dns_name(&self) -> Option<&str> {
        self.magic_dns_name.as_deref()
    }

    /// Returns all observed Tailscale IPs.
    pub fn ips(&self) -> &[String] {
        self.ips.as_slice()
    }

    /// Returns all observed Tailscale tags.
    pub fn tags(&self) -> &[String] {
        self.tags.as_slice()
    }

    /// Returns locally advertised Tailscale Services.
    pub fn services(&self) -> &[String] {
        self.services.as_slice()
    }
}

/// Collects local Tailscale IP, tag, MagicDNS, and service advertisement state.
pub async fn collect_tailscale_snapshot(
    command_timeout: Duration,
) -> AgentResult<TailscaleSnapshot> {
    let status = collect_tailscale_status(command_timeout).await?;
    let services = collect_tailscale_services(command_timeout).await?;
    if let Some(status) = status {
        let ipv4 = status
            .self_node
            .tailscale_ips
            .iter()
            .find(|value| value.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv4()))
            .cloned();
        return TailscaleSnapshot::new(
            ipv4,
            empty_string_as_none(status.self_node.host_name),
            empty_string_as_none(status.self_node.dns_name),
            status.self_node.tailscale_ips,
            status.self_node.tags,
            services,
        );
    }

    let mut command = Command::new("/usr/bin/tailscale");
    command.args(["ip", "-4"]);
    let output = timeout(command_timeout, crate::command::output(&mut command))
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    let ipv4 = if output.status.success() {
        let stdout = String::from_utf8(output.stdout).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        })?;
        stdout
            .lines()
            .next()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
    } else {
        None
    };
    let ips = ipv4.iter().cloned().collect::<Vec<_>>();
    TailscaleSnapshot::new(ipv4, None, None, ips, Vec::new(), services)
}

async fn collect_tailscale_status(
    command_timeout: Duration,
) -> AgentResult<Option<TailscaleStatusJson>> {
    let mut command = Command::new("/usr/bin/tailscale");
    command.args(["status", "--json"]);
    let output = timeout(command_timeout, crate::command::output(&mut command))
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8(output.stdout).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })?;
    serde_json::from_str::<TailscaleStatusJson>(stdout.as_str())
        .map(Some)
        .map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        })
}

async fn collect_tailscale_services(command_timeout: Duration) -> AgentResult<Vec<String>> {
    let mut command = Command::new("/usr/bin/tailscale");
    command.args(["serve", "status", "--json"]);
    let output = timeout(command_timeout, crate::command::output(&mut command))
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let stdout = String::from_utf8(output.stdout).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })?;
    let value = serde_json::from_str::<serde_json::Value>(stdout.as_str()).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })?;
    let mut services = Vec::new();
    collect_service_names(&value, &mut services, 0);
    services.sort();
    services.dedup();
    Ok(services)
}

fn collect_service_names(value: &serde_json::Value, services: &mut Vec<String>, depth: u8) {
    if depth > MAX_SERVICE_STATUS_RECURSION_DEPTH {
        return;
    }
    let next_depth = depth.saturating_add(1);
    match value {
        serde_json::Value::String(value) => {
            if value.starts_with("svc:") {
                services.push(value.to_owned());
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_service_names(value, services, next_depth);
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                if key.starts_with("svc:") {
                    services.push(key.to_owned());
                }
                collect_service_names(value, services, next_depth);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

#[derive(Deserialize)]
struct TailscaleStatusJson {
    #[serde(rename = "Self")]
    self_node: TailscaleSelfJson,
}

#[derive(Deserialize)]
struct TailscaleSelfJson {
    #[serde(rename = "HostName", default)]
    host_name: String,
    #[serde(rename = "DNSName", default)]
    dns_name: String,
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Vec<String>,
    #[serde(rename = "Tags", default)]
    tags: Vec<String>,
}

/// Advertises one approved Tailscale service through `tailscale serve`.
pub async fn advertise_service(service_name: &str, command_timeout: Duration) -> AgentResult<()> {
    validate_service_name(service_name)?;
    let mut command = Command::new("/usr/bin/tailscale");
    command.args(["serve", "advertise", service_name]);
    run_tailscale_command(command, command_timeout).await
}

/// Drains one approved Tailscale service through `tailscale serve`.
pub async fn drain_service(service_name: &str, command_timeout: Duration) -> AgentResult<()> {
    validate_service_name(service_name)?;
    let mut command = Command::new("/usr/bin/tailscale");
    command.args(["serve", "drain", service_name]);
    run_tailscale_command(command, command_timeout).await?;
    clear_service_config(service_name, command_timeout).await
}

/// One Tailscale Service endpoint to render into serve config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscaleServiceEndpoint {
    listen: String,
    upstream: String,
}

impl TailscaleServiceEndpoint {
    /// Constructs one validated service endpoint.
    pub fn new(listen: String, upstream: String) -> AgentResult<Self> {
        validate_endpoint_key(listen.as_str())?;
        validate_upstream(upstream.as_str())?;
        Ok(Self { listen, upstream })
    }

    /// Returns the serve-config endpoint key.
    pub fn listen(&self) -> &str {
        self.listen.as_str()
    }

    /// Returns the local upstream URL.
    pub fn upstream(&self) -> &str {
        self.upstream.as_str()
    }
}

/// One Tailscale Service definition rendered by the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscaleServiceDefinition {
    service_name: String,
    endpoints: Vec<TailscaleServiceEndpoint>,
}

impl TailscaleServiceDefinition {
    /// Constructs one validated Tailscale Service definition.
    pub fn new(
        service_name: String,
        endpoints: Vec<TailscaleServiceEndpoint>,
    ) -> AgentResult<Self> {
        validate_service_name(service_name.as_str())?;
        if !service_name.starts_with("svc:") || endpoints.is_empty() || endpoints.len() > 16 {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::InvalidIdentity,
            ));
        }
        Ok(Self {
            service_name,
            endpoints,
        })
    }

    /// Returns the service name.
    pub fn service_name(&self) -> &str {
        self.service_name.as_str()
    }

    /// Returns configured endpoints.
    pub fn endpoints(&self) -> &[TailscaleServiceEndpoint] {
        self.endpoints.as_slice()
    }
}

/// Renders, installs, and advertises service definitions.
///
/// NOTE: this path intentionally re-renders and reapplies the full serve config
/// every invocation (`tailscale serve set-config --all`). This can briefly recycle
/// existing advertised services while changing unrelated entries and may cause short
/// connection churn around CONFIGURE_DOCKER_SERVICE reconciliation.
pub async fn apply_service_configs(
    definitions: &[TailscaleServiceDefinition],
    command_timeout: Duration,
) -> AgentResult<()> {
    apply_service_configs_with_runner(
        Path::new(TAILSCALE_SERVE_CONFIG_PATH),
        &RealServeCommandRunner,
        definitions,
        command_timeout,
    )
    .await
}

/// Applies serve config and advertises each service through the supplied runner.
///
/// Advertisement is intentionally recovery-driven rather than all-or-nothing:
/// if a later advertise call fails, earlier services may already be advertised.
/// The installed serve config remains the source of local intent, and central
/// Hephaestus reconciliation should reissue the same idempotent action.
///
/// TODO: future optimization could diff current vs desired config and only
/// re-apply unchanged services to avoid transient churn.
async fn apply_service_configs_with_runner(
    path: &Path,
    runner: &impl ServeCommandRunner,
    definitions: &[TailscaleServiceDefinition],
    command_timeout: Duration,
) -> AgentResult<()> {
    if definitions.is_empty() {
        return Ok(());
    }
    let mut current = read_serve_config(path).await?;
    for definition in definitions {
        current.services.insert(
            definition.service_name().to_owned(),
            ServeServiceConfig::from_definition(definition),
        );
    }
    write_serve_config(path, &current).await?;
    runner.set_config(path, command_timeout).await?;
    for definition in definitions {
        if let Err(error) = runner
            .advertise(definition.service_name(), command_timeout)
            .await
        {
            tracing::warn!(
                service_name = definition.service_name(),
                reason = ?error.reason(),
                "tailscale service advertisement failed after serve config was installed"
            );
            return Err(error);
        }
    }
    Ok(())
}

/// Clears one service from the persisted serve config and applies the result.
pub async fn clear_service_config(
    service_name: &str,
    command_timeout: Duration,
) -> AgentResult<()> {
    validate_service_name(service_name)?;
    let path = Path::new(TAILSCALE_SERVE_CONFIG_PATH);
    let mut current = read_serve_config(path).await?;
    current.services.remove(service_name);
    write_serve_config(path, &current).await?;
    set_serve_config(path, command_timeout).await
}

fn render_serve_config(config: &ServeConfig) -> AgentResult<String> {
    serde_json::to_string_pretty(config)
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
}

trait ServeCommandRunner: Send + Sync {
    fn set_config<'a>(
        &'a self,
        path: &'a Path,
        command_timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = AgentResult<()>> + Send + 'a>>;

    fn advertise<'a>(
        &'a self,
        service_name: &'a str,
        command_timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = AgentResult<()>> + Send + 'a>>;
}

struct RealServeCommandRunner;

impl ServeCommandRunner for RealServeCommandRunner {
    fn set_config<'a>(
        &'a self,
        path: &'a Path,
        command_timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = AgentResult<()>> + Send + 'a>> {
        Box::pin(set_serve_config(path, command_timeout))
    }

    fn advertise<'a>(
        &'a self,
        service_name: &'a str,
        command_timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = AgentResult<()>> + Send + 'a>> {
        Box::pin(advertise_service(service_name, command_timeout))
    }
}

async fn read_serve_config(path: &Path) -> AgentResult<ServeConfig> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => serde_json::from_str(contents.as_str()).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ServeConfig::empty()),
        Err(_error) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::FileReadFailed,
        )),
    }
}

async fn write_serve_config(path: &Path, config: &ServeConfig) -> AgentResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    let rendered = render_serve_config(config)?;
    let temp_path = path.with_extension("json.tmp");
    tokio::fs::write(&temp_path, rendered.as_bytes())
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    tokio::fs::rename(&temp_path, path)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
}

async fn set_serve_config(path: &Path, command_timeout: Duration) -> AgentResult<()> {
    let mut command = Command::new("/usr/bin/tailscale");
    command.args(["serve", "set-config", "--all"]);
    command.arg(path);
    run_tailscale_command(command, command_timeout).await
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ServeConfig {
    version: String,
    services: BTreeMap<String, ServeServiceConfig>,
}

impl ServeConfig {
    fn empty() -> Self {
        Self {
            version: "0.0.1".to_owned(),
            services: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ServeServiceConfig {
    endpoints: BTreeMap<String, String>,
}

impl ServeServiceConfig {
    fn from_definition(definition: &TailscaleServiceDefinition) -> Self {
        let mut endpoints = BTreeMap::new();
        for endpoint in definition.endpoints() {
            endpoints.insert(endpoint.listen().to_owned(), endpoint.upstream().to_owned());
        }
        Self { endpoints }
    }
}

async fn run_tailscale_command(mut command: Command, command_timeout: Duration) -> AgentResult<()> {
    let output = timeout(command_timeout, crate::command::output(&mut command))
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        ))
    }
}

fn validate_ip(value: &str) -> AgentResult<()> {
    value.parse::<IpAddr>().map(|_ip| ()).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })
}

fn validate_identity(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || value.len() > 253
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

fn validate_service_name(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':'))
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    Ok(())
}

fn validate_endpoint_key(value: &str) -> AgentResult<()> {
    let Some(port) = value.strip_prefix("tcp:") else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    };
    validate_port(port)
}

fn validate_upstream(value: &str) -> AgentResult<()> {
    let Some(rest) = value.strip_prefix("tcp://") else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    };
    let Some((host, port)) = rest.rsplit_once(':') else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    };
    if !matches!(host, "127.0.0.1" | "::1") {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    validate_port(port)
}

fn validate_port(value: &str) -> AgentResult<()> {
    let port = value
        .parse::<u16>()
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
    if port == 0 {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidNumber,
        ));
    }
    Ok(())
}

fn empty_string_as_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests;
