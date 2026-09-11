// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Safe structured-log helpers for server runtime lifecycle and infrastructure errors.
//!
//! # Examples
//!
//! ```no_run
//! use std::time::Duration;
//!
//! use reallyme_server_kit::config::{
//!     LogFormat, MetricsIdleTimeout, ObservabilityConfig, ServiceEnvironment,
//! };
//! use reallyme_server_kit::observability::{
//!     log_observability_startup_summary, log_service_starting, observability_startup_summary,
//! };
//! use reallyme_server_kit::startup::ServerName;
//! use reallyme_server_kit::version::BuildInfo;
//!
//! let server_name = ServerName::new("reallyme-api").expect("valid server name");
//! let build_info = BuildInfo::new(server_name.clone());
//! let observability = ObservabilityConfig::new(
//!     ServiceEnvironment::Local,
//!     LogFormat::PlainText,
//!     false,
//!     "reallyme_server_kit=info".to_owned(),
//!     MetricsIdleTimeout::new(Duration::from_secs(30)).expect("valid metrics timeout"),
//! )
//! .expect("valid observability config");
//!
//! log_service_starting(&server_name, &build_info);
//! log_observability_startup_summary(&observability_startup_summary(
//!     &server_name,
//!     reallyme_server_kit::startup::DeploymentRegion::default(),
//!     &observability,
//! ));
//! ```

use crate::config::BindAddress;
use crate::runtime::{AppName, ServerRuntimePhase};
use crate::shutdown::ShutdownReason;
use crate::startup::{ServerName, TaskName};
use crate::task::TaskExecutionErrorKind;
use crate::transport::{RequestId, TraceId};
use crate::version::BuildInfo;

struct OptionalRequestId(Option<RequestId>);

impl std::fmt::Display for OptionalRequestId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => value.fmt(formatter),
            None => formatter.write_str("absent"),
        }
    }
}

struct OptionalTraceId(Option<TraceId>);

impl std::fmt::Display for OptionalTraceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => value.fmt(formatter),
            None => formatter.write_str("absent"),
        }
    }
}

use super::startup::ObservabilityStartupSummary;

/// Low-cardinality infrastructure error kind used in structured logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Configuration or startup validation failure.
    Configuration,
    /// Transport-layer failure.
    Transport,
    /// Timeout or deadline exhaustion.
    Timeout,
    /// Authentication failure.
    Authentication,
    /// Authorization failure.
    Authorization,
    /// Downstream dependency failure.
    DependencyUnavailable,
    /// Internal infrastructure failure.
    Internal,
}

impl ErrorKind {
    /// Returns the stable structured-log value for this error kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::Transport => "transport",
            Self::Timeout => "timeout",
            Self::Authentication => "authentication",
            Self::Authorization => "authorization",
            Self::DependencyUnavailable => "dependency_unavailable",
            Self::Internal => "internal",
        }
    }
}

/// Emits the initial startup log for a server process.
///
/// The payload is intentionally limited to safe build identity fields.
pub fn log_service_starting(server_name: &ServerName, build_info: &BuildInfo) {
    tracing::info!(
        service.name = server_name.as_str(),
        service.version = build_info.service_version(),
        build.git_sha = build_info.git_sha_or_unknown(),
        build.timestamp = build_info.build_timestamp_or_unknown(),
        build.profile = build_info.build_profile().unwrap_or("unknown"),
        "server process starting"
    );
}

/// Emits one safe observability startup summary log.
///
/// This helper accepts only the redacted summary type so callers cannot
/// accidentally log raw configuration values from ad hoc startup code.
pub fn log_observability_startup_summary(summary: &ObservabilityStartupSummary) {
    tracing::info!(
        service.name = summary.server_name(),
        server.region = summary.deployment_region().as_str(),
        deployment.region = summary.deployment_region().as_str(),
        observability.log_format = ?summary.log_format(),
        observability.tracing_filter_configured = summary.tracing_filter_configured(),
        observability.metrics_idle_timeout_secs = summary.metrics_idle_timeout().as_duration().as_secs(),
        "startup config summary"
    );
}

/// Emits a structured log for each app registered in the server runtime.
pub fn log_runtime_app_enabled(server_name: &ServerName, app_name: &AppName) {
    tracing::info!(
        service.name = server_name.as_str(),
        app.name = app_name.as_str(),
        "runtime app enabled"
    );
}

/// Emits the deterministic startup order chosen after dependency resolution.
pub fn log_runtime_app_startup_order(
    server_name: &ServerName,
    app_name: &AppName,
    startup_order: usize,
) {
    tracing::info!(
        service.name = server_name.as_str(),
        app.name = app_name.as_str(),
        app.startup_order = startup_order,
        "runtime app startup order resolved"
    );
}

/// Emits a structured log before a startup readiness check runs.
pub fn log_runtime_startup_check_started(server_name: &ServerName, check_name: &TaskName) {
    tracing::info!(
        service.name = server_name.as_str(),
        task.name = check_name.as_str(),
        "runtime startup check started"
    );
}

/// Emits a structured log after a startup readiness check succeeds.
pub fn log_runtime_startup_check_completed(server_name: &ServerName, check_name: &TaskName) {
    tracing::info!(
        service.name = server_name.as_str(),
        task.name = check_name.as_str(),
        "runtime startup check completed"
    );
}

/// Emits a structured log after a startup readiness check fails.
pub fn log_runtime_startup_check_failed(
    server_name: &ServerName,
    check_name: &TaskName,
    kind: TaskExecutionErrorKind,
) {
    tracing::error!(
        service.name = server_name.as_str(),
        task.name = check_name.as_str(),
        error.kind = kind.as_str(),
        "runtime startup check failed"
    );
}

/// Emits a structured log before an app cleanup hook starts.
pub fn log_runtime_app_cleanup_started(
    server_name: &ServerName,
    app_name: &AppName,
    hook_name: &TaskName,
) {
    tracing::info!(
        service.name = server_name.as_str(),
        app.name = app_name.as_str(),
        task.name = hook_name.as_str(),
        "runtime app cleanup started"
    );
}

/// Emits a structured log after an app cleanup hook completes.
pub fn log_runtime_app_cleanup_completed(
    server_name: &ServerName,
    app_name: &AppName,
    hook_name: &TaskName,
) {
    tracing::info!(
        service.name = server_name.as_str(),
        app.name = app_name.as_str(),
        task.name = hook_name.as_str(),
        "runtime app cleanup completed"
    );
}

/// Emits a structured log when an app cleanup hook fails.
pub fn log_runtime_app_cleanup_failed(
    server_name: &ServerName,
    app_name: &AppName,
    hook_name: &TaskName,
    kind: TaskExecutionErrorKind,
) {
    tracing::error!(
        service.name = server_name.as_str(),
        app.name = app_name.as_str(),
        task.name = hook_name.as_str(),
        error.kind = kind.as_str(),
        "runtime app cleanup failed"
    );
}

/// Emits a structured log when a server intentionally runs only operational routes.
pub fn log_no_runtime_apps_enabled(server_name: &ServerName) {
    tracing::info!(
        service.name = server_name.as_str(),
        "no runtime apps enabled"
    );
}

/// Emits a structured log after the HTTP listener has bound successfully.
pub fn log_http_listener_started(server_name: &ServerName, bind_address: BindAddress) {
    tracing::info!(
        service.name = server_name.as_str(),
        network.transport = "http",
        bind.address = %bind_address.as_socket_addr(),
        "http listener started"
    );
}

/// Emits a structured log after a gRPC listener has bound successfully.
pub fn log_grpc_listener_started(
    server_name: &ServerName,
    task_name: &TaskName,
    bind_address: BindAddress,
) {
    tracing::info!(
        service.name = server_name.as_str(),
        task.name = task_name.as_str(),
        network.transport = "grpc",
        bind.address = %bind_address.as_socket_addr(),
        "grpc listener started"
    );
}

/// Emits a structured log when the runtime flips readiness to ready.
pub fn log_service_ready(server_name: &ServerName) {
    tracing::info!(
        service.name = server_name.as_str(),
        readiness.state = "ready",
        "server process ready"
    );
}

/// Emits a low-cardinality runtime phase transition log.
pub fn log_runtime_phase_transition(server_name: &ServerName, phase: ServerRuntimePhase) {
    tracing::info!(
        service.name = server_name.as_str(),
        runtime.phase = phase.as_str(),
        "runtime phase changed"
    );
}

/// Emits the structured shutdown-request log.
pub fn log_shutdown_requested(server_name: &ServerName, reason: ShutdownReason) {
    tracing::info!(
        service.name = server_name.as_str(),
        shutdown.reason = shutdown_reason_value(reason),
        "shutdown requested"
    );
}

/// Emits the structured shutdown-complete log.
pub fn log_shutdown_completed(server_name: &ServerName, reason: ShutdownReason) {
    tracing::info!(
        service.name = server_name.as_str(),
        shutdown.reason = shutdown_reason_value(reason),
        "shutdown completed"
    );
}

/// Emits a safe infrastructure error log without leaking internal diagnostics.
///
/// The message should be a stable operator-facing description, not a formatted
/// string derived from secrets, request bodies, or downstream error text.
pub fn log_error(
    kind: ErrorKind,
    message: &'static str,
    request_id: Option<RequestId>,
    trace_id: Option<TraceId>,
) {
    tracing::error!(
        error.kind = kind.as_str(),
        request.id = %OptionalRequestId(request_id),
        trace.id = %OptionalTraceId(trace_id),
        "{message}"
    );
}

const fn shutdown_reason_value(reason: ShutdownReason) -> &'static str {
    match reason {
        ShutdownReason::CtrlC => "ctrl_c",
        ShutdownReason::Sigterm => "sigterm",
        ShutdownReason::Drop => "drop",
        ShutdownReason::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorKind, shutdown_reason_value};
    use crate::shutdown::ShutdownReason;

    #[test]
    fn error_kind_values_remain_stable() {
        assert_eq!(ErrorKind::Configuration.as_str(), "configuration");
        assert_eq!(ErrorKind::Transport.as_str(), "transport");
        assert_eq!(ErrorKind::Timeout.as_str(), "timeout");
        assert_eq!(ErrorKind::Authentication.as_str(), "authentication");
        assert_eq!(ErrorKind::Authorization.as_str(), "authorization");
        assert_eq!(
            ErrorKind::DependencyUnavailable.as_str(),
            "dependency_unavailable"
        );
        assert_eq!(ErrorKind::Internal.as_str(), "internal");
    }

    #[test]
    fn shutdown_reason_values_remain_stable() {
        assert_eq!(shutdown_reason_value(ShutdownReason::CtrlC), "ctrl_c");
        assert_eq!(shutdown_reason_value(ShutdownReason::Sigterm), "sigterm");
        assert_eq!(shutdown_reason_value(ShutdownReason::Unknown), "unknown");
    }
}
