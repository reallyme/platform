// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
#[path = "ids_tests.rs"]
mod tests;
