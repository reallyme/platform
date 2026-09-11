// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Strong health and log-summary types for Hephaestus fleet diagnostics.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

const MAX_SERVER_HOST_BYTES: usize = 253;
const MAX_HEALTH_URL_BYTES: usize = 512;
const MAX_PROVISIONING_DETAIL_BYTES: usize = 256;

/// Hephaestus domain validation failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum HephaestusDomainError {
    /// The value was empty after normalization.
    #[error("hephaestus value is empty")]
    Empty,
    /// The value exceeded its fixed maximum length.
    #[error("hephaestus value is too long")]
    TooLong,
    /// The value contained a disallowed character or shape.
    #[error("hephaestus value contains an invalid character")]
    InvalidCharacter,
    /// The numeric value was outside its accepted range.
    #[error("hephaestus value is outside the accepted range")]
    InvalidNumber,
}

/// Hostname or IP address of a server to probe.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct HephaestusServerHost(String);

impl HephaestusServerHost {
    /// Constructs a validated probe host.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_SERVER_HOST_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_host_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated host.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for HephaestusServerHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusServerHost")
            .field(&self.0)
            .finish()
    }
}

/// TCP port to probe on a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct HephaestusServerPort(u16);

impl HephaestusServerPort {
    /// Constructs a validated server port.
    pub const fn new(value: u16) -> Result<Self, HephaestusDomainError> {
        if value == 0 {
            return Err(HephaestusDomainError::InvalidNumber);
        }
        Ok(Self(value))
    }

    /// Returns the port number.
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl<'de> Deserialize<'de> for HephaestusServerPort {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// HTTP health endpoint to probe.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct HephaestusHealthUrl(String);

impl HephaestusHealthUrl {
    /// Constructs a validated HTTP health URL.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_HEALTH_URL_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if (!trimmed.starts_with("http://") && !trimmed.starts_with("https://"))
            || !trimmed.bytes().all(is_url_byte)
        {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the URL string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for HephaestusHealthUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusHealthUrl")
            .field(&self.0)
            .finish()
    }
}

/// V1 server health probe target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HephaestusHealthCheckTarget {
    host: HephaestusServerHost,
    app_port: Option<HephaestusServerPort>,
    http_health_url: Option<HephaestusHealthUrl>,
}

impl HephaestusHealthCheckTarget {
    /// Constructs a health check target.
    pub const fn new(
        host: HephaestusServerHost,
        app_port: Option<HephaestusServerPort>,
        http_health_url: Option<HephaestusHealthUrl>,
    ) -> Self {
        Self {
            host,
            app_port,
            http_health_url,
        }
    }

    /// Returns the server host.
    pub const fn host(&self) -> &HephaestusServerHost {
        &self.host
    }

    /// Returns the optional app port probe.
    pub const fn app_port(&self) -> Option<HephaestusServerPort> {
        self.app_port
    }

    /// Returns the optional HTTP health URL.
    pub const fn http_health_url(&self) -> Option<&HephaestusHealthUrl> {
        self.http_health_url.as_ref()
    }
}

/// Status of a V1 health probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthProbeStatus {
    /// Probe completed successfully.
    Ok,
    /// Probe failed.
    Failed,
    /// Probe was not requested.
    NotChecked,
}

/// Provisioning event kind for the current V1 job model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningEventKind {
    /// Server creation was requested.
    Requested,
    /// Provider instance creation started.
    InstanceCreateStarted,
    /// Provider instance creation succeeded.
    InstanceCreateSucceeded,
    /// Provider instance creation failed.
    InstanceCreateFailed,
    /// DNS reconciliation started.
    DnsReconcileStarted,
    /// DNS reconciliation succeeded.
    DnsReconcileSucceeded,
    /// DNS reconciliation failed.
    DnsReconcileFailed,
    /// Health verification started.
    HealthCheckStarted,
    /// Health verification succeeded.
    HealthCheckSucceeded,
    /// Health verification failed.
    HealthCheckFailed,
    /// The node agent completed an authenticated boot report.
    BootReported,
    /// The node is considered ready after boot reporting.
    Ready,
}

impl ProvisioningEventKind {
    /// Returns the stable event name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::InstanceCreateStarted => "instance_create_started",
            Self::InstanceCreateSucceeded => "instance_create_succeeded",
            Self::InstanceCreateFailed => "instance_create_failed",
            Self::DnsReconcileStarted => "dns_reconcile_started",
            Self::DnsReconcileSucceeded => "dns_reconcile_succeeded",
            Self::DnsReconcileFailed => "dns_reconcile_failed",
            Self::HealthCheckStarted => "health_check_started",
            Self::HealthCheckSucceeded => "health_check_succeeded",
            Self::HealthCheckFailed => "health_check_failed",
            Self::BootReported => "boot_reported",
            Self::Ready => "ready",
        }
    }
}

/// Last provisioning event snapshot shown beside V1 health.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningEventSnapshot {
    kind: ProvisioningEventKind,
    detail: Option<String>,
}

impl ProvisioningEventSnapshot {
    /// Constructs a provisioning event snapshot.
    pub fn new(
        kind: ProvisioningEventKind,
        detail: Option<String>,
    ) -> Result<Self, HephaestusDomainError> {
        let detail = detail.map(validate_detail).transpose()?;
        Ok(Self { kind, detail })
    }

    /// Returns the event kind.
    pub const fn kind(&self) -> ProvisioningEventKind {
        self.kind
    }

    /// Returns the optional stable detail.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

/// Complete V1 server health report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HephaestusServerHealthReport {
    server_id: String,
    server_reachable: HealthProbeStatus,
    app_port_open: HealthProbeStatus,
    http_health_ok: HealthProbeStatus,
    last_provisioning_event: Option<ProvisioningEventSnapshot>,
}

impl HephaestusServerHealthReport {
    /// Constructs a server health report.
    pub fn new(
        server_id: impl Into<String>,
        server_reachable: HealthProbeStatus,
        app_port_open: HealthProbeStatus,
        http_health_ok: HealthProbeStatus,
        last_provisioning_event: Option<ProvisioningEventSnapshot>,
    ) -> Result<Self, HephaestusDomainError> {
        Ok(Self {
            server_id: validate_detail(server_id.into())?,
            server_reachable,
            app_port_open,
            http_health_ok,
            last_provisioning_event,
        })
    }

    /// Returns the server identifier associated with the report.
    pub fn server_id(&self) -> &str {
        self.server_id.as_str()
    }

    /// Returns whether the server accepted a basic reachability probe.
    pub const fn server_reachable(&self) -> HealthProbeStatus {
        self.server_reachable
    }

    /// Returns whether the requested application TCP port was open.
    pub const fn app_port_open(&self) -> HealthProbeStatus {
        self.app_port_open
    }

    /// Returns whether the requested HTTP health endpoint returned success.
    pub const fn http_health_ok(&self) -> HealthProbeStatus {
        self.http_health_ok
    }

    /// Returns the latest provisioning event snapshot known to Hephaestus.
    pub const fn last_provisioning_event(&self) -> Option<&ProvisioningEventSnapshot> {
        self.last_provisioning_event.as_ref()
    }
}

/// Stable server-kit structured log event known to Hephaestus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HephaestusServerKitLogEvent {
    /// Event message emitted by server-kit tracing.
    pub message: &'static str,
    /// Primary stable fields operators should expect with the event.
    pub fields: &'static [&'static str],
}

/// Catalog of server-kit structured log events Hephaestus can display before
/// log ingestion is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HephaestusServerKitLogEventCatalog {
    /// Known event entries.
    pub events: &'static [HephaestusServerKitLogEvent],
}

/// Returns the stable server-kit log-event catalog.
pub const fn server_kit_log_event_catalog() -> HephaestusServerKitLogEventCatalog {
    HephaestusServerKitLogEventCatalog {
        events: &[
            HephaestusServerKitLogEvent {
                message: "tracing initialized",
                fields: &["service.name"],
            },
            HephaestusServerKitLogEvent {
                message: "server process starting",
                fields: &[
                    "service.name",
                    "service.version",
                    "build.git_sha",
                    "build.timestamp",
                    "build.profile",
                ],
            },
            HephaestusServerKitLogEvent {
                message: "startup config summary",
                fields: &[
                    "service.name",
                    "server.region",
                    "deployment.region",
                    "observability.log_format",
                    "observability.tracing_filter_configured",
                    "observability.metrics_idle_timeout_secs",
                ],
            },
            HephaestusServerKitLogEvent {
                message: "runtime app enabled",
                fields: &["service.name", "app.name"],
            },
            HephaestusServerKitLogEvent {
                message: "runtime app startup order resolved",
                fields: &["service.name", "app.name", "app.startup_order"],
            },
            HephaestusServerKitLogEvent {
                message: "runtime startup check started",
                fields: &["service.name", "task.name"],
            },
            HephaestusServerKitLogEvent {
                message: "runtime startup check completed",
                fields: &["service.name", "task.name"],
            },
            HephaestusServerKitLogEvent {
                message: "runtime startup check failed",
                fields: &["service.name", "task.name", "error.kind"],
            },
            HephaestusServerKitLogEvent {
                message: "runtime app cleanup started",
                fields: &["service.name", "app.name", "task.name"],
            },
            HephaestusServerKitLogEvent {
                message: "runtime app cleanup completed",
                fields: &["service.name", "app.name", "task.name"],
            },
            HephaestusServerKitLogEvent {
                message: "runtime app cleanup failed",
                fields: &["service.name", "app.name", "task.name", "error.kind"],
            },
            HephaestusServerKitLogEvent {
                message: "no runtime apps enabled",
                fields: &["service.name"],
            },
            HephaestusServerKitLogEvent {
                message: "http listener started",
                fields: &["service.name", "network.transport", "bind.address"],
            },
            HephaestusServerKitLogEvent {
                message: "grpc listener started",
                fields: &[
                    "service.name",
                    "task.name",
                    "network.transport",
                    "bind.address",
                ],
            },
            HephaestusServerKitLogEvent {
                message: "server process ready",
                fields: &["service.name", "readiness.state"],
            },
            HephaestusServerKitLogEvent {
                message: "runtime phase changed",
                fields: &["service.name", "runtime.phase"],
            },
            HephaestusServerKitLogEvent {
                message: "shutdown requested",
                fields: &["service.name", "shutdown.reason"],
            },
            HephaestusServerKitLogEvent {
                message: "shutdown completed",
                fields: &["service.name", "shutdown.reason"],
            },
            HephaestusServerKitLogEvent {
                message: "caller-provided static infrastructure error message",
                fields: &["error.kind", "request.id", "trace.id"],
            },
        ],
    }
}

fn validate_detail(value: String) -> Result<String, HephaestusDomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(HephaestusDomainError::Empty);
    }
    if trimmed.len() > MAX_PROVISIONING_DETAIL_BYTES {
        return Err(HephaestusDomainError::TooLong);
    }
    if !trimmed.bytes().all(is_detail_byte) {
        return Err(HephaestusDomainError::InvalidCharacter);
    }
    Ok(trimmed.to_owned())
}

fn is_host_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':')
}

fn is_url_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'.' | b'-' | b'_' | b'/' | b':' | b'?' | b'&' | b'=' | b'%' | b'+' | b'#'
        )
}

fn is_detail_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b' ' | b'.' | b'-' | b'_' | b'/' | b':' | b'@' | b'+' | b'=' | b',' | b'#'
        )
}

#[cfg(test)]
mod tests {
    use super::{HephaestusHealthUrl, HephaestusServerPort, server_kit_log_event_catalog};

    #[test]
    fn health_url_requires_http_scheme() {
        assert!(HephaestusHealthUrl::new("ftp://example.test/healthz").is_err());
        assert!(HephaestusHealthUrl::new("http://example.test/healthz").is_ok());
    }

    #[test]
    fn server_port_rejects_zero() {
        assert!(HephaestusServerPort::new(0).is_err());
        assert!(HephaestusServerPort::new(8721).is_ok());
    }

    #[test]
    fn server_kit_log_catalog_includes_ready_event() {
        assert!(
            server_kit_log_event_catalog()
                .events
                .iter()
                .any(|event| event.message == "server process ready")
        );
    }
}
