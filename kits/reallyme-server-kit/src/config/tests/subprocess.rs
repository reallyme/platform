// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::process::{Command, Output};
use std::time::Duration;

use crate::config::{ConfigError, ConfigValueErrorReason, LogFormat, ServiceEnvironment};

use super::fixtures::{load_process_test_config, test_env_var_name};

#[test]
fn config_parses_valid_env() {
    let output = spawn_config_test(
        "config::tests::subprocess::subprocess_config_parses_valid_env",
        &[
            ("REALLYME_CONFIG_TEST_MODE", "parse-valid"),
            ("TEST_SERVICE_ENVIRONMENT", "dev"),
            ("TEST_HTTP_BIND_ADDR", "127.0.0.1:8080"),
            ("TEST_GRPC_BIND_ADDR", "127.0.0.1:9090"),
            ("TEST_REQUEST_TIMEOUT_MILLIS", "5000"),
            ("TEST_REQUEST_BODY_LIMIT_BYTES", "1048576"),
            ("TEST_LOG_FORMAT", "json"),
            ("TEST_TRACING_FILTER", "info,reallyme_server_kit=debug"),
            ("TEST_EMIT_SPAN_EVENTS", "true"),
            ("TEST_API_TOKEN", "super-secret-token"),
        ],
    );

    assert_subprocess_success(&output);
}

#[test]
fn config_rejects_invalid_bind_address() {
    let output = spawn_config_test(
        "config::tests::subprocess::subprocess_config_rejects_invalid_bind_address",
        &[
            ("REALLYME_CONFIG_TEST_MODE", "invalid-bind"),
            ("TEST_SERVICE_ENVIRONMENT", "dev"),
            ("TEST_HTTP_BIND_ADDR", "not-an-address"),
            ("TEST_GRPC_BIND_ADDR", "127.0.0.1:9090"),
            ("TEST_REQUEST_TIMEOUT_MILLIS", "5000"),
            ("TEST_REQUEST_BODY_LIMIT_BYTES", "1048576"),
            ("TEST_LOG_FORMAT", "json"),
            ("TEST_TRACING_FILTER", "info"),
            ("TEST_API_TOKEN", "super-secret-token"),
        ],
    );

    assert_subprocess_success(&output);
}

#[test]
fn production_critical_config_fails_closed() {
    let output = spawn_config_test(
        "config::tests::subprocess::subprocess_production_critical_config_fails_closed",
        &[
            ("REALLYME_CONFIG_TEST_MODE", "prod-critical-missing"),
            ("TEST_SERVICE_ENVIRONMENT", "prod"),
            ("TEST_HTTP_BIND_ADDR", "127.0.0.1:8080"),
            ("TEST_GRPC_BIND_ADDR", "127.0.0.1:9090"),
            ("TEST_REQUEST_TIMEOUT_MILLIS", "5000"),
            ("TEST_REQUEST_BODY_LIMIT_BYTES", "1048576"),
            ("TEST_LOG_FORMAT", "json"),
            ("TEST_TRACING_FILTER", "info"),
        ],
    );

    assert_subprocess_success(&output);
}

#[test]
fn subprocess_config_parses_valid_env() {
    if std::env::var("REALLYME_CONFIG_TEST_MODE").ok().as_deref() != Some("parse-valid") {
        return;
    }

    let config = load_process_test_config().expect("valid subprocess config fixture");
    assert_eq!(
        config.http.bind_address().as_socket_addr(),
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080))
    );
    assert_eq!(
        config.grpc.bind_address().as_socket_addr(),
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9090))
    );
    assert_eq!(
        config.http.timeout().request_timeout().as_duration(),
        Duration::from_secs(5)
    );
    assert_eq!(
        config.http.body_limit().request_body_limit().as_usize(),
        1_048_576
    );
    assert_eq!(
        config.observability.service_environment(),
        ServiceEnvironment::Dev
    );
    assert_eq!(config.observability.log_format(), LogFormat::Json);
    assert!(config.observability.emit_span_events());
    assert_eq!(config.api_token.expose_secret(), "super-secret-token");
}

#[test]
fn subprocess_config_rejects_invalid_bind_address() {
    if std::env::var("REALLYME_CONFIG_TEST_MODE").ok().as_deref() != Some("invalid-bind") {
        return;
    }

    let result = load_process_test_config();

    assert!(matches!(
        result,
        Err(ConfigError::InvalidValue {
            name,
            reason: ConfigValueErrorReason::InvalidSocketAddress,
        }) if name == test_env_var_name("TEST_HTTP_BIND_ADDR")
    ));
}

#[test]
fn subprocess_production_critical_config_fails_closed() {
    if std::env::var("REALLYME_CONFIG_TEST_MODE").ok().as_deref() != Some("prod-critical-missing") {
        return;
    }

    let result = load_process_test_config();

    assert!(matches!(
        result,
        Err(ConfigError::MissingProductionCriticalVariable {
            name,
            service_environment: ServiceEnvironment::Prod,
        }) if name == test_env_var_name("TEST_API_TOKEN")
    ));
}

fn spawn_config_test(test_name: &str, variables: &[(&str, &str)]) -> Output {
    let current_exe = std::env::current_exe().expect("current test executable path");

    let mut command = Command::new(current_exe);
    command.env_clear();
    command.arg("--exact").arg(test_name).arg("--nocapture");

    for (name, value) in variables {
        command.env(name, value);
    }

    command
        .output()
        .expect("subprocess config test output should be captured")
}

fn assert_subprocess_success(output: &Output) {
    assert!(
        output.status.success(),
        "subprocess failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
