// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use crate::health::ReadinessState;
use crate::http::HttpListenerName;
use crate::observability::ObservabilityError;
use crate::observability::metrics::{
    HttpMethodLabel, HttpRejectionReason, HttpStatusClass, MetricName, MetricRouteTemplateLabel,
    RouteTemplate, RuntimeAppFailureOutcome, RuntimeQueueLabel, install_prometheus_recorder,
    record_http_request_completed, record_http_request_outcome,
    record_http_request_rejected_for_route_template, record_rate_limit_buckets_live,
    record_rate_limit_mutex_poisoned, record_readiness_state, record_runtime_app_cleanup_failure,
    record_runtime_app_startup_failure, record_runtime_phase, record_runtime_queue_saturation,
    record_startup_info,
};
use crate::runtime::ServerRuntimePhase;
use crate::startup::ServerName;
use crate::version::BuildInfo;

use super::fixtures::{SUBPROCESS_METRICS_TEST_ENV, test_config};

#[test]
fn metrics_subprocess_worker() {
    let Some(action) = std::env::var_os(SUBPROCESS_METRICS_TEST_ENV) else {
        return;
    };

    let config = test_config();

    match action.to_string_lossy().as_ref() {
        "install-and-render" => {
            let exporter =
                install_prometheus_recorder(&config).expect("prometheus recorder should install");
            let route = RouteTemplate::new("/readyz").expect("valid route template");
            let server_name = ServerName::new("reallyme-api").expect("valid server name");
            let build_info = BuildInfo::new(server_name.clone());

            record_http_request_outcome(
                HttpMethodLabel::Get,
                route,
                HttpStatusClass::Success,
                Duration::from_millis(15),
            );
            record_http_request_outcome(
                HttpMethodLabel::Get,
                route,
                HttpStatusClass::ServerError,
                Duration::from_millis(20),
            );
            record_http_request_completed(
                HttpMethodLabel::Head,
                route,
                HttpStatusClass::Redirection,
                Duration::from_millis(5),
            );
            let listener_name =
                HttpListenerName::new("public").expect("test listener name is valid");
            let route_label = MetricRouteTemplateLabel::from_static("/readyz");
            record_http_request_rejected_for_route_template(
                &listener_name,
                HttpMethodLabel::Get,
                &route_label,
                HttpRejectionReason::ConcurrencyLimit,
            );
            record_http_request_rejected_for_route_template(
                &listener_name,
                HttpMethodLabel::Get,
                &route_label,
                HttpRejectionReason::UntrustedProxyHeaders,
            );
            record_readiness_state(ReadinessState::Ready);
            record_runtime_phase(ServerRuntimePhase::Serving);
            record_runtime_queue_saturation(RuntimeQueueLabel::TaskChannel);
            record_runtime_app_startup_failure(RuntimeAppFailureOutcome::Failed);
            record_runtime_app_cleanup_failure(RuntimeAppFailureOutcome::TimedOut);
            record_startup_info(&server_name, &build_info);
            record_rate_limit_buckets_live(Arc::<str>::from("public"), 7);
            record_rate_limit_mutex_poisoned();

            let rendered = exporter.render();

            assert!(rendered.contains(MetricName::HttpRequestCount.as_str()));
            assert!(rendered.contains(MetricName::HttpRequestDurationSeconds.as_str()));
            assert!(rendered.contains(MetricName::HttpErrorCount.as_str()));
            assert!(rendered.contains(MetricName::HttpRejectedCount.as_str()));
            assert!(rendered.contains(MetricName::RuntimeQueueSaturationCount.as_str()));
            assert!(rendered.contains(MetricName::RuntimeAppStartupFailureCount.as_str()));
            assert!(rendered.contains(MetricName::RuntimeAppCleanupFailureCount.as_str()));
            assert!(rendered.contains(MetricName::StartupInfo.as_str()));
            assert!(rendered.contains(MetricName::ReadinessState.as_str()));
            assert!(rendered.contains(MetricName::RuntimePhase.as_str()));
            assert!(rendered.contains(MetricName::RateLimitBucketsLive.as_str()));
            assert!(rendered.contains(MetricName::RateLimitMutexPoisoned.as_str()));
            assert!(rendered.contains("route=\"/readyz\""));
            assert!(rendered.contains("listener_name=\"public\""));
            assert!(rendered.contains("listener_name=\"__unknown\""));
            assert!(rendered.contains("reason=\"concurrency_limit\""));
            assert!(rendered.contains("reason=\"untrusted_proxy_headers\""));
            assert!(rendered.contains("server_name=\"reallyme-api\""));
            assert!(rendered.contains("service_version="));
            assert!(rendered.contains("git_sha="));
            assert!(rendered.contains("queue=\"task_channel\""));
            assert!(rendered.contains("outcome=\"failed\""));
            assert!(rendered.contains("outcome=\"timed_out\""));
            assert!(!rendered.contains("build_timestamp="));
        }
        "duplicate-install" => {
            let first = install_prometheus_recorder(&config);
            let second = install_prometheus_recorder(&config);

            assert!(first.is_ok());
            assert!(matches!(
                second,
                Err(ObservabilityError::MetricsRecorderAlreadyInstalled)
            ));
        }
        value => panic!("unsupported metrics subprocess action: {value}"),
    }
}

#[test]
fn metrics_install_and_render_helper_work_in_subprocess() {
    let current_exe = std::env::current_exe().expect("current test binary path should resolve");

    let output = Command::new(&current_exe)
        .arg("--exact")
        .arg("observability::metrics::tests::exporter::metrics_subprocess_worker")
        .env_clear()
        .env(SUBPROCESS_METRICS_TEST_ENV, "install-and-render")
        .output()
        .expect("metrics subprocess should launch");

    assert!(
        output.status.success(),
        "metrics subprocess failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn duplicate_metrics_install_is_rejected_in_subprocess() {
    let current_exe = std::env::current_exe().expect("current test binary path should resolve");

    let output = Command::new(&current_exe)
        .arg("--exact")
        .arg("observability::metrics::tests::exporter::metrics_subprocess_worker")
        .env_clear()
        .env(SUBPROCESS_METRICS_TEST_ENV, "duplicate-install")
        .output()
        .expect("metrics subprocess should launch");

    assert!(
        output.status.success(),
        "metrics subprocess failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
