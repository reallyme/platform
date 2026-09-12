// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tracing and metrics bootstrap helpers.

mod error;
mod fields;
#[cfg(feature = "http")]
mod logging;
#[cfg(feature = "metrics")]
mod metrics;
#[cfg(feature = "http")]
mod spans;
mod startup;
mod tracing;

#[cfg(feature = "http")]
pub use crate::http::{IdentifierHeaderError, X_REQUEST_ID, X_TRACE_ID};
pub use crate::transport::{RequestId, TraceId};
pub use error::{MetricLabelError, ObservabilityError};
pub use fields::{
    ERROR_KIND_FIELD, GRPC_METHOD_FIELD, GRPC_SERVICE_FIELD, GRPC_STATUS_CODE_FIELD,
    HTTP_METHOD_FIELD, HTTP_ROUTE_FIELD, HTTP_STATUS_CLASS_FIELD, REQUEST_ID_FIELD,
    SERVER_NAME_FIELD, SERVICE_VERSION_FIELD, SHUTDOWN_REASON_FIELD, TRACE_ID_FIELD,
};
#[cfg(feature = "http")]
pub use logging::{
    ErrorKind, log_error, log_grpc_listener_started, log_http_listener_started,
    log_no_runtime_apps_enabled, log_observability_startup_summary,
    log_runtime_app_cleanup_completed, log_runtime_app_cleanup_failed,
    log_runtime_app_cleanup_started, log_runtime_app_enabled, log_runtime_app_startup_order,
    log_runtime_critical_task_failed, log_runtime_critical_task_ready,
    log_runtime_critical_task_started, log_runtime_phase_transition,
    log_runtime_startup_check_completed, log_runtime_startup_check_failed,
    log_runtime_startup_check_started, log_service_ready, log_service_starting,
    log_shutdown_completed, log_shutdown_requested,
};
#[cfg(all(feature = "http", feature = "metrics"))]
pub(crate) use metrics::MetricRouteTemplateLabel;
#[cfg(all(feature = "http", feature = "metrics"))]
pub use metrics::record_runtime_phase;
#[cfg(feature = "metrics")]
pub use metrics::{
    HttpMethodLabel, HttpRateLimitOutcome, HttpRejectionReason, HttpStatusClass, MetricName,
    MetricsExporter, PrometheusExposureModel, RouteTemplate, RuntimeAppFailureOutcome,
    RuntimeQueueLabel, TransportLabel, UNKNOWN_LISTENER_NAME, UNKNOWN_ROUTE_TEMPLATE,
    install_prometheus_recorder, record_http_request_completed, record_http_request_error,
    record_http_request_outcome, record_rate_limit_buckets_live, record_rate_limit_mutex_poisoned,
    record_readiness_state, record_runtime_app_cleanup_failure, record_runtime_app_startup_failure,
    record_runtime_queue_saturation, record_startup_info,
};
#[cfg(all(feature = "http", feature = "metrics"))]
pub(crate) use metrics::{
    record_http_rate_limit_decision_for_route_template_with_transport,
    record_http_request_outcome_for_route_template,
    record_http_request_rejected_for_route_template,
    record_http_request_rejected_for_route_template_with_transport,
    record_service_discovery_resolve,
};
#[cfg(feature = "http")]
pub use spans::{background_task_span, grpc_request_span, http_request_span};
pub use startup::{ObservabilityStartupSummary, observability_startup_summary};
pub use tracing::{
    TracingOutputMode, init_json_tracing, init_pretty_tracing, init_tracing,
    opentelemetry_bridge_layer,
};
