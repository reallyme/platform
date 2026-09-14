// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
