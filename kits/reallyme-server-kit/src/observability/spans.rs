// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Canonical tracing span helpers.
//!
//! # Examples
//!
//! ```rust
//! use axum::http::Method;
//!
//! use reallyme_server_kit::observability::{grpc_request_span, http_request_span};
//! use reallyme_server_kit::transport::{RequestId, TraceId};
//!
//! let request_id = RequestId::generate();
//! let trace_id = TraceId::generate();
//!
//! let http_span = http_request_span(&Method::GET, Some("/readyz"), Some(request_id), Some(trace_id));
//! let grpc_span = grpc_request_span(
//!     "reallyme.api.HealthService",
//!     "Check",
//!     Some(request_id),
//!     Some(trace_id),
//! );
//!
//! assert_eq!(
//!     http_span.metadata().expect("span metadata should exist").name(),
//!     "http_request"
//! );
//! assert_eq!(
//!     grpc_span.metadata().expect("span metadata should exist").name(),
//!     "grpc_request"
//! );
//! ```

use axum::http::Method;
use tracing::Span;

use crate::startup::TaskName;
use crate::transport::{RequestId, TraceId};

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

/// Creates the standard background-task span.
pub fn background_task_span(task_name: &TaskName) -> Span {
    tracing::info_span!("background_task", task.name = task_name.as_str())
}

/// Creates the standard HTTP request span.
///
/// Callers must pass a stable route template from router metadata, not a raw
/// URI path. When no matched route template is available, `"<unmatched>"` is
/// used as the low-cardinality fallback.
pub fn http_request_span(
    method: &Method,
    route_template: Option<&str>,
    request_id: Option<RequestId>,
    trace_id: Option<TraceId>,
) -> Span {
    tracing::info_span!(
        "http_request",
        http.method = %method,
        http.route = route_template.unwrap_or("<unmatched>"),
        request.id = %OptionalRequestId(request_id),
        trace.id = %OptionalTraceId(trace_id),
        listener.name = tracing::field::Empty,
        client.ip = tracing::field::Empty,
        http.external_host = tracing::field::Empty,
        http.external_proto = tracing::field::Empty,
        http.external_port = tracing::field::Empty,
        http.external_origin = tracing::field::Empty,
    )
}

/// Creates the standard gRPC request span.
///
/// Service and method values should come from generated gRPC service metadata
/// or static service registration, never from caller-provided metadata.
pub fn grpc_request_span(
    grpc_service: &str,
    grpc_method: &str,
    request_id: Option<RequestId>,
    trace_id: Option<TraceId>,
) -> Span {
    tracing::info_span!(
        "grpc_request",
        grpc.service = grpc_service,
        grpc.method = grpc_method,
        request.id = %OptionalRequestId(request_id),
        trace.id = %OptionalTraceId(trace_id),
    )
}

#[cfg(test)]
mod tests {
    use axum::http::Method;

    use super::{background_task_span, grpc_request_span, http_request_span};
    use crate::startup::TaskName;
    use crate::transport::{RequestId, TraceId};

    #[test]
    fn span_names_remain_stable() {
        let task_name = TaskName::new("worker").expect("valid task name");
        let request_id = RequestId::generate();
        let trace_id = TraceId::generate();

        assert_eq!(
            background_task_span(&task_name)
                .metadata()
                .expect("span metadata should exist")
                .name(),
            "background_task"
        );
        assert_eq!(
            http_request_span(
                &Method::GET,
                Some("/readyz"),
                Some(request_id),
                Some(trace_id)
            )
            .metadata()
            .expect("span metadata should exist")
            .name(),
            "http_request"
        );
        assert_eq!(
            grpc_request_span(
                "reallyme.api.UserService",
                "GetUser",
                Some(request_id),
                Some(trace_id)
            )
            .metadata()
            .expect("span metadata should exist")
            .name(),
            "grpc_request"
        );
    }
}
