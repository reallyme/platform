// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::time::Duration;

use metrics::{SharedString, counter, gauge, histogram};

use crate::health::ReadinessState;
#[cfg(feature = "http")]
use crate::http::HttpListenerName;
#[cfg(feature = "http")]
use crate::runtime::ServerRuntimePhase;
use crate::startup::ServerName;
use crate::version::BuildInfo;

use super::labels::{
    HttpMethodLabel, HttpStatusClass, METRIC_LABEL_GIT_SHA, METRIC_LABEL_LISTENER_NAME,
    METRIC_LABEL_METHOD, METRIC_LABEL_ROUTE, METRIC_LABEL_RUNTIME_OUTCOME,
    METRIC_LABEL_RUNTIME_QUEUE, METRIC_LABEL_SERVER_NAME, METRIC_LABEL_SERVICE_VERSION,
    METRIC_LABEL_STATUS_CLASS, RouteTemplate, RuntimeAppFailureOutcome, RuntimeQueueLabel,
    UNKNOWN_LISTENER_NAME,
};
#[cfg(feature = "http")]
use super::labels::{
    HttpRateLimitOutcome, HttpRejectionReason, METRIC_LABEL_DISCOVERY_OUTCOME,
    METRIC_LABEL_DISCOVERY_SOURCE, METRIC_LABEL_RATE_LIMIT_OUTCOME, METRIC_LABEL_RATE_LIMIT_TIER,
    METRIC_LABEL_REJECTION_REASON, METRIC_LABEL_TRANSPORT, MetricRouteTemplateLabel,
    TransportLabel,
};
use super::names::MetricName;

#[cfg(feature = "http")]
fn listener_metric_label(listener_name: &HttpListenerName) -> SharedString {
    // metrics 0.24's SharedString has an explicit shared Arc<str> variant.
    // Use it directly so listener labels retain by refcount, not String allocation.
    SharedString::from_shared(listener_name.clone_shared())
}

/// Records a completed HTTP request using only low-cardinality transport labels.
pub fn record_http_request_completed(
    method: HttpMethodLabel,
    route_template: RouteTemplate,
    status_class: HttpStatusClass,
    duration: Duration,
) {
    counter!(
        MetricName::HttpRequestCount.as_str(),
        METRIC_LABEL_LISTENER_NAME => UNKNOWN_LISTENER_NAME,
        METRIC_LABEL_METHOD => method.as_str(),
        METRIC_LABEL_ROUTE => route_template.as_str(),
        METRIC_LABEL_STATUS_CLASS => status_class.as_str()
    )
    .increment(1);

    histogram!(
        MetricName::HttpRequestDurationSeconds.as_str(),
        METRIC_LABEL_LISTENER_NAME => UNKNOWN_LISTENER_NAME,
        METRIC_LABEL_METHOD => method.as_str(),
        METRIC_LABEL_ROUTE => route_template.as_str(),
        METRIC_LABEL_STATUS_CLASS => status_class.as_str()
    )
    .record(duration.as_secs_f64());
}

#[cfg(feature = "http")]
pub(crate) fn record_http_request_completed_for_route_template(
    listener_name: &HttpListenerName,
    method: HttpMethodLabel,
    route_template: &MetricRouteTemplateLabel,
    status_class: HttpStatusClass,
    duration: Duration,
) {
    counter!(
        MetricName::HttpRequestCount.as_str(),
        METRIC_LABEL_LISTENER_NAME => listener_metric_label(listener_name),
        METRIC_LABEL_METHOD => method.as_str(),
        METRIC_LABEL_ROUTE => route_template.clone_shared(),
        METRIC_LABEL_STATUS_CLASS => status_class.as_str()
    )
    .increment(1);

    histogram!(
        MetricName::HttpRequestDurationSeconds.as_str(),
        METRIC_LABEL_LISTENER_NAME => listener_metric_label(listener_name),
        METRIC_LABEL_METHOD => method.as_str(),
        METRIC_LABEL_ROUTE => route_template.clone_shared(),
        METRIC_LABEL_STATUS_CLASS => status_class.as_str()
    )
    .record(duration.as_secs_f64());
}

/// Records an HTTP request that should be counted as an error.
///
/// Apps should treat this as a companion metric to
/// [`record_http_request_completed`], not a replacement. Requests should still
/// be counted in the completed-request metrics, and this helper should be used
/// only to count the error subset explicitly.
pub fn record_http_request_error(
    method: HttpMethodLabel,
    route_template: RouteTemplate,
    status_class: HttpStatusClass,
) {
    counter!(
        MetricName::HttpErrorCount.as_str(),
        METRIC_LABEL_LISTENER_NAME => UNKNOWN_LISTENER_NAME,
        METRIC_LABEL_METHOD => method.as_str(),
        METRIC_LABEL_ROUTE => route_template.as_str(),
        METRIC_LABEL_STATUS_CLASS => status_class.as_str()
    )
    .increment(1);
}

#[cfg(feature = "http")]
pub(crate) fn record_http_request_error_for_route_template(
    listener_name: &HttpListenerName,
    method: HttpMethodLabel,
    route_template: &MetricRouteTemplateLabel,
    status_class: HttpStatusClass,
) {
    counter!(
        MetricName::HttpErrorCount.as_str(),
        METRIC_LABEL_LISTENER_NAME => listener_metric_label(listener_name),
        METRIC_LABEL_METHOD => method.as_str(),
        METRIC_LABEL_ROUTE => route_template.clone_shared(),
        METRIC_LABEL_STATUS_CLASS => status_class.as_str()
    )
    .increment(1);
}

/// Records the standard HTTP request metrics for one completed request.
///
/// This helper always records request count and duration. It also increments
/// the error counter when the provided status class represents a `4xx` or `5xx`
/// outcome so app code can avoid drifting into inconsistent counting.
pub fn record_http_request_outcome(
    method: HttpMethodLabel,
    route_template: RouteTemplate,
    status_class: HttpStatusClass,
    duration: Duration,
) {
    record_http_request_completed(method, route_template, status_class, duration);

    if matches!(
        status_class,
        HttpStatusClass::ClientError | HttpStatusClass::ServerError
    ) {
        record_http_request_error(method, route_template, status_class);
    }
}

#[cfg(feature = "http")]
pub(crate) fn record_http_request_outcome_for_route_template(
    listener_name: &HttpListenerName,
    method: HttpMethodLabel,
    route_template: &MetricRouteTemplateLabel,
    status_class: HttpStatusClass,
    duration: Duration,
) {
    record_http_request_completed_for_route_template(
        listener_name,
        method,
        route_template,
        status_class,
        duration,
    );

    if matches!(
        status_class,
        HttpStatusClass::ClientError | HttpStatusClass::ServerError
    ) {
        record_http_request_error_for_route_template(
            listener_name,
            method,
            route_template,
            status_class,
        );
    }
}

#[cfg(feature = "http")]
pub(crate) fn record_http_request_rejected_for_route_template(
    listener_name: &HttpListenerName,
    method: HttpMethodLabel,
    route_template: &MetricRouteTemplateLabel,
    reason: HttpRejectionReason,
) {
    record_http_request_rejected_for_route_template_with_transport(
        listener_metric_label(listener_name),
        TransportLabel::Http,
        method,
        route_template,
        reason,
    );
}

#[cfg(feature = "http")]
pub(crate) fn record_http_request_rejected_for_route_template_with_transport(
    listener_name: SharedString,
    transport: TransportLabel,
    method: HttpMethodLabel,
    route_template: &MetricRouteTemplateLabel,
    reason: HttpRejectionReason,
) {
    counter!(
        MetricName::HttpRejectedCount.as_str(),
        METRIC_LABEL_LISTENER_NAME => listener_name,
        METRIC_LABEL_TRANSPORT => transport.as_str(),
        METRIC_LABEL_METHOD => method.as_str(),
        METRIC_LABEL_ROUTE => route_template.clone_shared(),
        METRIC_LABEL_REJECTION_REASON => reason.as_str()
    )
    .increment(1);
}

#[cfg(feature = "http")]
pub(crate) fn record_http_rate_limit_decision_for_route_template_with_transport(
    listener_name: SharedString,
    transport: TransportLabel,
    method: HttpMethodLabel,
    route_template: &MetricRouteTemplateLabel,
    rate_limit_tier: SharedString,
    outcome: HttpRateLimitOutcome,
) {
    counter!(
        MetricName::HttpRateLimitDecisionCount.as_str(),
        METRIC_LABEL_LISTENER_NAME => listener_name,
        METRIC_LABEL_TRANSPORT => transport.as_str(),
        METRIC_LABEL_METHOD => method.as_str(),
        METRIC_LABEL_ROUTE => route_template.clone_shared(),
        METRIC_LABEL_RATE_LIMIT_TIER => rate_limit_tier,
        METRIC_LABEL_RATE_LIMIT_OUTCOME => outcome.as_str()
    )
    .increment(1);
}

/// Records saturation of a bounded runtime queue.
pub fn record_runtime_queue_saturation(queue: RuntimeQueueLabel) {
    counter!(
        MetricName::RuntimeQueueSaturationCount.as_str(),
        METRIC_LABEL_RUNTIME_QUEUE => queue.as_str()
    )
    .increment(1);
}

/// Records a runtime app startup-check failure.
pub fn record_runtime_app_startup_failure(outcome: RuntimeAppFailureOutcome) {
    counter!(
        MetricName::RuntimeAppStartupFailureCount.as_str(),
        METRIC_LABEL_RUNTIME_OUTCOME => outcome.as_str()
    )
    .increment(1);
}

/// Records a runtime app cleanup failure or timeout.
pub fn record_runtime_app_cleanup_failure(outcome: RuntimeAppFailureOutcome) {
    counter!(
        MetricName::RuntimeAppCleanupFailureCount.as_str(),
        METRIC_LABEL_RUNTIME_OUTCOME => outcome.as_str()
    )
    .increment(1);
}

/// Records a service locator resolution attempt by the winning source.
#[cfg(feature = "http")]
pub(crate) fn record_service_discovery_resolve(source: &'static str, outcome: &'static str) {
    counter!(
        MetricName::ServiceDiscoveryResolveCount.as_str(),
        METRIC_LABEL_DISCOVERY_SOURCE => source,
        METRIC_LABEL_DISCOVERY_OUTCOME => outcome
    )
    .increment(1);
}

/// Records the current number of live in-memory rate-limit buckets for a listener.
pub fn record_rate_limit_buckets_live(listener_name: Arc<str>, live_buckets: usize) {
    gauge!(
        MetricName::RateLimitBucketsLive.as_str(),
        METRIC_LABEL_LISTENER_NAME => SharedString::from_shared(listener_name),
    )
    .set(live_buckets as f64);
}

/// Records recovery from a poisoned rate-limit registry mutex.
pub fn record_rate_limit_mutex_poisoned() {
    counter!(MetricName::RateLimitMutexPoisoned.as_str()).increment(1);
}

/// Records immutable startup/build information for the running server process.
///
/// `startup_info` is the only approved metric in `reallyme-server-kit` that
/// may carry deploy/build identity labels. Even here, labels must remain
/// tightly bounded and stable enough for operational build introspection. This
/// helper intentionally excludes build timestamps to avoid unnecessary time
/// series churn across rebuilds.
pub fn record_startup_info(server_name: &ServerName, build_info: &BuildInfo) {
    gauge!(
        MetricName::StartupInfo.as_str(),
        METRIC_LABEL_SERVER_NAME => server_name.as_str().to_owned(),
        METRIC_LABEL_SERVICE_VERSION => build_info.service_version(),
        METRIC_LABEL_GIT_SHA => build_info.git_sha_or_unknown()
    )
    .set(1.0);
}

/// Records the current readiness state.
pub fn record_readiness_state(state: ReadinessState) {
    let value = match state {
        ReadinessState::Ready => 1.0,
        ReadinessState::NotReady => 0.0,
    };

    gauge!(MetricName::ReadinessState.as_str()).set(value);
}

/// Records the current runtime lifecycle phase.
#[cfg(feature = "http")]
pub fn record_runtime_phase(phase: ServerRuntimePhase) {
    gauge!(MetricName::RuntimePhase.as_str()).set(phase.metric_value());
}
