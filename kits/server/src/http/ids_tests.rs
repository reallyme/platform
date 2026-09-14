// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::{Extensions, HeaderMap, HeaderValue};

use super::{
    IdentifierHeaderError, request_id_from_extensions, request_id_from_headers,
    trace_id_from_extensions, trace_id_from_headers,
};
use crate::transport::{RequestId, TraceId};

#[test]
fn request_id_round_trips_through_header_value() {
    let request_id = RequestId::generate();
    let header_value = request_id
        .to_header_value()
        .expect("generated request ID should produce a header value");
    let parsed = RequestId::try_from(&header_value);
    assert_eq!(parsed, Ok(request_id));
}

#[test]
fn trace_id_round_trips_through_header_value() {
    let trace_id = TraceId::generate();
    let header_value = trace_id
        .to_header_value()
        .expect("generated trace ID should produce a header value");
    let parsed = TraceId::try_from(&header_value);
    assert_eq!(parsed, Ok(trace_id));
}

#[test]
fn request_id_rejects_invalid_header_value() {
    let header_value = HeaderValue::from_static("not-a-uuid");

    let result = RequestId::try_from(&header_value);

    assert_eq!(result, Err(IdentifierHeaderError::InvalidUuid));
}

#[test]
fn request_and_trace_id_helpers_extract_valid_values() {
    let request_id = RequestId::generate();
    let trace_id = TraceId::generate();
    let request_header = request_id
        .to_header_value()
        .expect("generated request ID should produce a header value");
    let trace_header = trace_id
        .to_header_value()
        .expect("generated trace ID should produce a header value");
    let mut headers = HeaderMap::new();

    headers.insert(super::X_REQUEST_ID, request_header);
    headers.insert(super::X_TRACE_ID, trace_header);

    assert_eq!(request_id_from_headers(&headers), Some(request_id));
    assert_eq!(trace_id_from_headers(&headers), Some(trace_id));
}

#[test]
fn extension_helpers_return_stored_identifiers() {
    let request_id = RequestId::generate();
    let trace_id = TraceId::generate();
    let mut extensions = Extensions::new();

    extensions.insert(request_id);
    extensions.insert(trace_id);

    assert_eq!(request_id_from_extensions(&extensions), Some(request_id));
    assert_eq!(trace_id_from_extensions(&extensions), Some(trace_id));
}
