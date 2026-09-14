// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::process::Command;
use std::time::Duration;

use axum::Router;
use axum::http::{StatusCode, header};
use axum::routing::get;

use super::super::{
    METRICS_PATH, apply_standard_router_layers, apply_standard_router_layers_with_request_logging,
    metrics_route,
};
use super::fixtures::{
    SUBPROCESS_HTTP_ROUTES_TEST_ENV, failing_route, test_http_config, test_observability_config,
};
use crate::config::{HttpRequestLogMode, HttpRequestLoggingConfig};
use crate::health::ReadinessState;
use crate::http::TestServer;
use crate::observability::{MetricName, install_prometheus_recorder, record_readiness_state};

#[test]
fn metrics_subprocess_worker() {
    let Some(action) = std::env::var_os(SUBPROCESS_HTTP_ROUTES_TEST_ENV) else {
        return;
    };

    match action.to_string_lossy().as_ref() {
        "metrics-route" => {
            let exporter = install_prometheus_recorder(&test_observability_config())
                .expect("prometheus recorder should install");
            record_readiness_state(ReadinessState::NotReady);
            let app = Router::new().route(METRICS_PATH, metrics_route(exporter));
            let server = TestServer::new(app);

            let response = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime should build")
                .block_on(async { server.get(METRICS_PATH).await });

            response.assert_status_ok();
            response.assert_header(
                header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            );
            assert!(response.text().contains("# HELP"));
        }
        "standard-layer-metrics" => {
            let exporter = install_prometheus_recorder(&test_observability_config())
                .expect("prometheus recorder should install");
            let app = apply_standard_router_layers(
                Router::new()
                    .route("/hello", get(|| async { StatusCode::OK }))
                    .route("/fail", get(failing_route)),
                &test_http_config(Duration::from_secs(1), 1024),
            );
            let server = TestServer::new(app);
            let rendered = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime should build")
                .block_on(async {
                    let success = server.get("/hello").await;
                    success.assert_status_ok();

                    let failure = server.get("/fail").await;
                    failure.assert_status(StatusCode::INTERNAL_SERVER_ERROR);

                    exporter.render()
                });

            assert!(rendered.contains(MetricName::HttpRequestCount.as_str()));
            assert!(rendered.contains(MetricName::HttpRequestDurationSeconds.as_str()));
            assert!(rendered.contains(MetricName::HttpErrorCount.as_str()));
            assert!(rendered.contains("method=\"GET\""));
            assert!(rendered.contains("route=\"/hello\""));
            assert!(rendered.contains("route=\"/fail\""));
            assert!(rendered.contains("status_class=\"2xx\""));
            assert!(rendered.contains("status_class=\"5xx\""));
            assert!(!rendered.contains("route=\"/hello?"));
        }
        "standard-layer-metrics-disabled-request-logs" => {
            let exporter = install_prometheus_recorder(&test_observability_config())
                .expect("prometheus recorder should install");
            let request_logging =
                HttpRequestLoggingConfig::new(HttpRequestLogMode::Disabled, None, None)
                    .expect("disabled request log mode should be valid");
            let app = apply_standard_router_layers_with_request_logging(
                Router::new().route("/hello", get(|| async { StatusCode::OK })),
                &test_http_config(Duration::from_secs(1), 1024),
                request_logging,
            );
            let server = TestServer::new(app);
            let rendered = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime should build")
                .block_on(async {
                    let success = server.get("/hello").await;
                    success.assert_status_ok();
                    exporter.render()
                });

            assert!(rendered.contains(MetricName::HttpRequestCount.as_str()));
            assert!(rendered.contains(MetricName::HttpRequestDurationSeconds.as_str()));
            assert!(rendered.contains("route=\"/hello\""));
            assert!(rendered.contains("status_class=\"2xx\""));
        }
        value => panic!("unsupported http routes subprocess action: {value}"),
    }
}

#[test]
fn metrics_route_renders_prometheus_payload_in_subprocess() {
    let current_exe = std::env::current_exe().expect("current test binary path should resolve");

    let output = Command::new(&current_exe)
        .arg("--exact")
        .arg("http::routes::tests::metrics::metrics_subprocess_worker")
        .env_clear()
        .env(SUBPROCESS_HTTP_ROUTES_TEST_ENV, "metrics-route")
        .output()
        .expect("metrics subprocess should launch");

    assert!(
        output.status.success(),
        "http routes subprocess failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn standard_layers_emit_http_metrics_in_subprocess() {
    let current_exe = std::env::current_exe().expect("current test binary path should resolve");

    let output = Command::new(&current_exe)
        .arg("--exact")
        .arg("http::routes::tests::metrics::metrics_subprocess_worker")
        .env_clear()
        .env(SUBPROCESS_HTTP_ROUTES_TEST_ENV, "standard-layer-metrics")
        .output()
        .expect("http routes subprocess should launch");

    assert!(
        output.status.success(),
        "http routes subprocess failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn standard_layers_emit_http_metrics_when_request_completion_logs_are_disabled() {
    let current_exe = std::env::current_exe().expect("current test binary path should resolve");

    let output = Command::new(&current_exe)
        .arg("--exact")
        .arg("http::routes::tests::metrics::metrics_subprocess_worker")
        .env_clear()
        .env(
            SUBPROCESS_HTTP_ROUTES_TEST_ENV,
            "standard-layer-metrics-disabled-request-logs",
        )
        .output()
        .expect("http routes subprocess should launch");

    assert!(
        output.status.success(),
        "http routes subprocess failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
