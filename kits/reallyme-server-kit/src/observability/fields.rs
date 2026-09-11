// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use super::{
        ERROR_KIND_FIELD, GRPC_METHOD_FIELD, GRPC_SERVICE_FIELD, GRPC_STATUS_CODE_FIELD,
        HTTP_METHOD_FIELD, HTTP_ROUTE_FIELD, HTTP_STATUS_CLASS_FIELD, REQUEST_ID_FIELD,
        SERVER_NAME_FIELD, SERVICE_VERSION_FIELD, SHUTDOWN_REASON_FIELD, TRACE_ID_FIELD,
    };

    #[test]
    fn field_names_remain_stable() {
        assert_eq!(SERVER_NAME_FIELD, "service.name");
        assert_eq!(SERVICE_VERSION_FIELD, "service.version");
        assert_eq!(REQUEST_ID_FIELD, "request.id");
        assert_eq!(TRACE_ID_FIELD, "trace.id");
        assert_eq!(HTTP_METHOD_FIELD, "http.method");
        assert_eq!(HTTP_ROUTE_FIELD, "http.route");
        assert_eq!(HTTP_STATUS_CLASS_FIELD, "http.status_class");
        assert_eq!(GRPC_SERVICE_FIELD, "grpc.service");
        assert_eq!(GRPC_METHOD_FIELD, "grpc.method");
        assert_eq!(GRPC_STATUS_CODE_FIELD, "grpc.status_code");
        assert_eq!(SHUTDOWN_REASON_FIELD, "shutdown.reason");
        assert_eq!(ERROR_KIND_FIELD, "error.kind");
    }
}
