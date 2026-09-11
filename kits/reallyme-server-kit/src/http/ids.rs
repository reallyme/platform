// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::{Extensions, HeaderMap, HeaderName, HeaderValue};
use thiserror::Error;
use uuid::Uuid;

use crate::transport::{IdentifierValueError, RequestId, TraceId};

/// Standard request identifier header.
pub const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
/// Standard trace identifier header.
pub const X_TRACE_ID: HeaderName = HeaderName::from_static("x-trace-id");

/// Identifier parsing and formatting failures for request and trace headers.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierHeaderError {
    /// The header value is not valid visible ASCII text.
    #[error("identifier header value is not valid ascii")]
    InvalidAscii,
    /// The header value is not a valid UUID.
    #[error("identifier header value is not a valid uuid")]
    InvalidUuid,
}

impl RequestId {
    /// Formats the identifier as an HTTP header value.
    pub fn to_header_value(self) -> Result<HeaderValue, IdentifierHeaderError> {
        uuid_header_value(self.into_uuid())
    }
}

impl TraceId {
    /// Formats the identifier as an HTTP header value.
    pub fn to_header_value(self) -> Result<HeaderValue, IdentifierHeaderError> {
        uuid_header_value(self.into_uuid())
    }
}

impl TryFrom<&HeaderValue> for RequestId {
    type Error = IdentifierHeaderError;

    fn try_from(value: &HeaderValue) -> Result<Self, Self::Error> {
        let value = value
            .to_str()
            .map_err(|_| IdentifierHeaderError::InvalidAscii)?;
        RequestId::parse_str(value).map_err(map_identifier_value_error)
    }
}

impl TryFrom<&HeaderValue> for TraceId {
    type Error = IdentifierHeaderError;

    fn try_from(value: &HeaderValue) -> Result<Self, Self::Error> {
        let value = value
            .to_str()
            .map_err(|_| IdentifierHeaderError::InvalidAscii)?;
        TraceId::parse_str(value).map_err(map_identifier_value_error)
    }
}

/// Extracts a validated request identifier from HTTP headers.
pub fn request_id_from_headers(headers: &HeaderMap) -> Option<RequestId> {
    headers
        .get(X_REQUEST_ID)
        .and_then(|value| RequestId::try_from(value).ok())
}

/// Extracts a validated trace identifier from HTTP headers.
pub fn trace_id_from_headers(headers: &HeaderMap) -> Option<TraceId> {
    headers
        .get(X_TRACE_ID)
        .and_then(|value| TraceId::try_from(value).ok())
}

/// Extracts a request identifier previously stored in request or response
/// extensions.
pub fn request_id_from_extensions(extensions: &Extensions) -> Option<RequestId> {
    extensions.get::<RequestId>().copied()
}

/// Extracts a trace identifier previously stored in request or response
/// extensions.
pub fn trace_id_from_extensions(extensions: &Extensions) -> Option<TraceId> {
    extensions.get::<TraceId>().copied()
}

fn map_identifier_value_error(error: IdentifierValueError) -> IdentifierHeaderError {
    match error {
        IdentifierValueError::InvalidUuid => IdentifierHeaderError::InvalidUuid,
    }
}

fn uuid_header_value(value: Uuid) -> Result<HeaderValue, IdentifierHeaderError> {
    let mut buffer = Uuid::encode_buffer();
    let encoded = value.hyphenated().encode_lower(&mut buffer);

    HeaderValue::from_str(encoded).map_err(|_| IdentifierHeaderError::InvalidAscii)
}

#[cfg(test)]
mod tests {
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
}
