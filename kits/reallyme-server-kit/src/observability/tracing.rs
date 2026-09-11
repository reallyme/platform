// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use std::process::Command;
    use std::time::Duration;

    use super::{TracingOutputMode, parse_env_filter, span_events_for_config};
    use crate::config::{LogFormat, MetricsIdleTimeout, ObservabilityConfig, ServiceEnvironment};
    use crate::observability::ObservabilityError;
    use crate::startup::ServerName;

    const SUBPROCESS_TRACING_TEST_ENV: &str = "REALLYME_SERVER_KIT_TRACING_SUBPROCESS";

    fn test_config(log_format: LogFormat, filter: &str) -> ObservabilityConfig {
        ObservabilityConfig::new(
            ServiceEnvironment::Local,
            log_format,
            true,
            filter.to_owned(),
            MetricsIdleTimeout::new(Duration::from_secs(30)).expect("valid metrics timeout"),
        )
        .expect("valid observability config")
    }

    #[test]
    fn log_format_maps_to_expected_output_mode() {
        assert_eq!(
            TracingOutputMode::from_log_format(LogFormat::Json),
            TracingOutputMode::Json
        );
        assert_eq!(
            TracingOutputMode::from_log_format(LogFormat::PlainText),
            TracingOutputMode::Pretty
        );
    }

    #[test]
    fn invalid_tracing_filter_is_rejected() {
        let result = parse_env_filter("[");

        assert!(matches!(
            result,
            Err(ObservabilityError::InvalidTracingFilter)
        ));
    }

    #[test]
    fn span_event_mapping_respects_config() {
        let enabled = test_config(LogFormat::Json, "reallyme_server_kit=info");
        let disabled = ObservabilityConfig::new(
            ServiceEnvironment::Local,
            LogFormat::Json,
            false,
            "reallyme_server_kit=info".to_owned(),
            MetricsIdleTimeout::new(Duration::from_secs(30)).expect("valid metrics timeout"),
        )
        .expect("valid observability config");

        assert_eq!(
            span_events_for_config(&enabled),
            tracing_subscriber::fmt::format::FmtSpan::NEW
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE
        );
        assert_eq!(
            span_events_for_config(&disabled),
            tracing_subscriber::fmt::format::FmtSpan::NONE
        );
    }

    #[test]
    fn tracing_subprocess_worker() {
        let Some(action) = std::env::var_os(SUBPROCESS_TRACING_TEST_ENV) else {
            return;
        };

        let server_name = ServerName::new("reallyme-api").expect("valid server name");
        let config = test_config(LogFormat::Json, "reallyme_server_kit=info");

        match action.to_string_lossy().as_ref() {
            "duplicate-init" => {
                let first = super::init_tracing(server_name.clone(), &config);
                let second = super::init_tracing(server_name, &config);

                assert_eq!(first, Ok(()));
                assert_eq!(second, Err(ObservabilityError::TracingAlreadyInitialized));
            }
            value => panic!("unsupported tracing subprocess action: {value}"),
        }
    }

    #[test]
    fn duplicate_tracing_init_is_rejected_in_subprocess() {
        let current_exe = std::env::current_exe().expect("current test binary path should resolve");

        let output = Command::new(current_exe)
            .arg("--exact")
            .arg("observability::tracing::tests::tracing_subprocess_worker")
            .env_clear()
            .env(SUBPROCESS_TRACING_TEST_ENV, "duplicate-init")
            .output()
            .expect("tracing subprocess should launch");

        assert!(
            output.status.success(),
            "tracing subprocess failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
