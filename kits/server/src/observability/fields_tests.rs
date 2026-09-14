// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
