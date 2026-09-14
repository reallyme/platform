// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Low-cardinality metrics primitives and Prometheus exporter helpers.

mod describe;
mod exporter;
mod labels;
mod names;
mod record;

#[cfg(test)]
mod tests;

pub use exporter::{MetricsExporter, PrometheusExposureModel, install_prometheus_recorder};
#[cfg(feature = "http")]
pub(crate) use labels::MetricRouteTemplateLabel;
pub use labels::{
    HttpMethodLabel, HttpRateLimitOutcome, HttpRejectionReason, HttpStatusClass, RouteTemplate,
    RuntimeAppFailureOutcome, RuntimeQueueLabel, TransportLabel, UNKNOWN_LISTENER_NAME,
    UNKNOWN_ROUTE_TEMPLATE,
};
pub use names::MetricName;
#[cfg(feature = "http")]
pub use record::record_runtime_phase;
pub use record::{
    record_http_request_completed, record_http_request_error, record_http_request_outcome,
    record_rate_limit_buckets_live, record_rate_limit_mutex_poisoned, record_readiness_state,
    record_runtime_app_cleanup_failure, record_runtime_app_startup_failure,
    record_runtime_queue_saturation, record_startup_info,
};

#[cfg(all(feature = "http", feature = "metrics"))]
pub(crate) use record::{
    record_http_rate_limit_decision_for_route_template_with_transport,
    record_http_request_outcome_for_route_template,
    record_http_request_rejected_for_route_template,
    record_http_request_rejected_for_route_template_with_transport,
    record_service_discovery_resolve,
};
