// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use opentelemetry::trace::Tracer;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry, fmt, registry};

use crate::config::{LogFormat, ObservabilityConfig};
use crate::startup::ServerName;

use super::error::ObservabilityError;

/// Supported tracing output modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TracingOutputMode {
    /// Structured JSON logs intended for production ingestion.
    Json,
    /// Compact human-readable logs intended for local development.
    Pretty,
}

impl TracingOutputMode {
    fn from_log_format(value: LogFormat) -> Self {
        match value {
            LogFormat::Json => Self::Json,
            LogFormat::PlainText => Self::Pretty,
        }
    }
}

fn parse_env_filter(value: &str) -> Result<EnvFilter, ObservabilityError> {
    EnvFilter::try_new(value).map_err(|_| ObservabilityError::InvalidTracingFilter)
}

fn span_events_for_config(config: &ObservabilityConfig) -> FmtSpan {
    if config.emit_span_events() {
        FmtSpan::NEW | FmtSpan::CLOSE
    } else {
        FmtSpan::NONE
    }
}

/// Installs the process-global tracing subscriber.
///
/// This function is intended to be called exactly once during startup before
/// any workload begins. Repeated installation attempts are rejected explicitly.
///
/// # Examples
///
/// ```no_run
/// use std::time::Duration;
///
/// use reallyme_server_kit::config::{LogFormat, MetricsIdleTimeout, ObservabilityConfig, ServiceEnvironment};
/// use reallyme_server_kit::observability::init_tracing;
/// use reallyme_server_kit::startup::ServerName;
///
/// let server_name = ServerName::new("reallyme-api").expect("valid server name");
/// let config = ObservabilityConfig::new(
///     ServiceEnvironment::Local,
///     LogFormat::PlainText,
///     false,
///     "reallyme_server_kit=debug".to_owned(),
///     MetricsIdleTimeout::new(Duration::from_secs(30)).expect("valid metrics timeout"),
/// )
/// .expect("valid observability config");
///
/// init_tracing(server_name, &config).expect("tracing should initialize");
/// ```
pub fn init_tracing(
    server_name: ServerName,
    config: &ObservabilityConfig,
) -> Result<(), ObservabilityError> {
    let env_filter = parse_env_filter(config.tracing_filter_directives())?;
    let span_events = span_events_for_config(config);

    match TracingOutputMode::from_log_format(config.log_format()) {
        TracingOutputMode::Json => init_json_tracing(env_filter, span_events)?,
        TracingOutputMode::Pretty => init_pretty_tracing(env_filter, span_events)?,
    }

    tracing::info!(service.name = server_name.as_str(), "tracing initialized");
    Ok(())
}

/// Builds a tracing OpenTelemetry bridge layer for future exporter wiring.
///
/// This keeps OpenTelemetry integration aligned with the platform tracing
/// standard without forcing app crates to construct ad hoc bridge layers.
pub fn opentelemetry_bridge_layer<T>(tracer: T) -> OpenTelemetryLayer<Registry, T>
where
    T: Tracer + Send + Sync + 'static,
    T::Span: Send + Sync,
{
    tracing_opentelemetry::layer().with_tracer(tracer)
}

/// Installs the JSON tracing subscriber.
pub fn init_json_tracing(
    env_filter: EnvFilter,
    span_events: FmtSpan,
) -> Result<(), ObservabilityError> {
    let fmt_layer = fmt::layer()
        .json()
        .with_writer(std::io::stdout)
        .with_current_span(true)
        .with_span_list(true)
        .with_span_events(span_events)
        .with_target(true)
        .with_ansi(false);

    registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init()
        .map_err(|_| ObservabilityError::TracingAlreadyInitialized)
}

/// Installs the pretty development tracing subscriber.
pub fn init_pretty_tracing(
    env_filter: EnvFilter,
    span_events: FmtSpan,
) -> Result<(), ObservabilityError> {
    // Use compact single-line formatting rather than `pretty()` so local
    // development logs stay close to production semantics: one event per line,
    // no source-file/line noise, and no blank multi-line rendering.
    let fmt_layer = fmt::layer()
        .compact()
        .with_writer(std::io::stdout)
        .with_span_events(span_events)
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .with_ansi(false);

    registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init()
        .map_err(|_| ObservabilityError::TracingAlreadyInitialized)
}

#[cfg(test)]
#[path = "tracing_tests.rs"]
mod tests;
