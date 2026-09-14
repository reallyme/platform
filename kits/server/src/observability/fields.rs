// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Stable structured-log field names used across the platform.
//!
//! These constants document the canonical field vocabulary for tracing-based
//! service logs and spans. Tracing macros still require literal field names, so
//! callers should usually prefer the typed helpers in sibling modules such as
//! `logging` and `spans` rather than reusing these constants directly.

/// `service.name`
pub const SERVER_NAME_FIELD: &str = "service.name";
/// `service.version`
pub const SERVICE_VERSION_FIELD: &str = "service.version";
/// `request.id`
pub const REQUEST_ID_FIELD: &str = "request.id";
/// `trace.id`
pub const TRACE_ID_FIELD: &str = "trace.id";
/// `http.method`
pub const HTTP_METHOD_FIELD: &str = "http.method";
/// `http.route`
pub const HTTP_ROUTE_FIELD: &str = "http.route";
/// `http.status_class`
pub const HTTP_STATUS_CLASS_FIELD: &str = "http.status_class";
/// `grpc.service`
pub const GRPC_SERVICE_FIELD: &str = "grpc.service";
/// `grpc.method`
pub const GRPC_METHOD_FIELD: &str = "grpc.method";
/// `grpc.status_code`
pub const GRPC_STATUS_CODE_FIELD: &str = "grpc.status_code";
/// `shutdown.reason`
pub const SHUTDOWN_REASON_FIELD: &str = "shutdown.reason";
/// `error.kind`
pub const ERROR_KIND_FIELD: &str = "error.kind";

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
