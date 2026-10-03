// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validated inputs for a bounded HTTPS exchange.

use std::time::Duration;

use reqwest::header::{HeaderName, HeaderValue};

use super::{
    HttpsTransportError, HttpsTransportErrorReason,
    origin::{MAXIMUM_TARGET_BYTES as MAXIMUM_ENCODED_TARGET_BYTES, validate_target},
};

/// Absolute ceiling for a single sensitive request body.
const MAXIMUM_HTTPS_REQUEST_BYTES: usize = 8 * 1_024 * 1_024;
/// Absolute ceiling for a single sensitive response body.
const MAXIMUM_HTTPS_RESPONSE_BYTES: usize = 16 * 1_024 * 1_024;
const MAXIMUM_RESPONSE_HEADER_BYTES: usize = 8 * 1_024;
const MAXIMUM_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// HTTP methods supported by the sensitive application transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpsMethod {
    /// Create or invoke a resource.
    Post,
    /// Replace an addressed resource.
    Put,
    /// Delete an addressed resource.
    Delete,
}

impl HttpsMethod {
    pub(super) const fn as_reqwest(self) -> reqwest::Method {
        match self {
            Self::Post => reqwest::Method::POST,
            Self::Put => reqwest::Method::PUT,
            Self::Delete => reqwest::Method::DELETE,
        }
    }
}

/// Caller-selected limits constrained by platform hard ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpsExchangeLimits {
    maximum_request_bytes: usize,
    maximum_response_bytes: usize,
}

impl HttpsExchangeLimits {
    /// Platform hard ceiling for a request body.
    pub const MAXIMUM_REQUEST_BYTES: usize = MAXIMUM_HTTPS_REQUEST_BYTES;
    /// Platform hard ceiling for a response body.
    pub const MAXIMUM_RESPONSE_BYTES: usize = MAXIMUM_HTTPS_RESPONSE_BYTES;

    /// Validates request and response byte limits.
    pub fn new(
        maximum_request_bytes: usize,
        maximum_response_bytes: usize,
    ) -> Result<Self, HttpsTransportError> {
        if maximum_request_bytes == 0
            || maximum_request_bytes > MAXIMUM_HTTPS_REQUEST_BYTES
            || maximum_response_bytes == 0
            || maximum_response_bytes > MAXIMUM_HTTPS_RESPONSE_BYTES
        {
            return Err(HttpsTransportError::local(
                HttpsTransportErrorReason::InvalidLimit,
            ));
        }
        Ok(Self {
            maximum_request_bytes,
            maximum_response_bytes,
        })
    }

    pub(super) const fn maximum_request_bytes(self) -> usize {
        self.maximum_request_bytes
    }

    pub(super) const fn maximum_response_bytes(self) -> usize {
        self.maximum_response_bytes
    }
}

/// One response header that an application adapter needs for protocol handling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapturedResponseHeader {
    name: &'static str,
    maximum_bytes: usize,
}

impl CapturedResponseHeader {
    /// Validates a response-header selection and its byte limit.
    pub fn new(name: &'static str, maximum_bytes: usize) -> Result<Self, HttpsTransportError> {
        HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
            HttpsTransportError::local(HttpsTransportErrorReason::InvalidHeaderName)
        })?;
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_RESPONSE_HEADER_BYTES {
            return Err(HttpsTransportError::local(
                HttpsTransportErrorReason::InvalidLimit,
            ));
        }
        Ok(Self {
            name,
            maximum_bytes,
        })
    }

    pub(super) fn header_name(self) -> Result<HeaderName, HttpsTransportError> {
        HeaderName::from_bytes(self.name.as_bytes())
            .map_err(|_| HttpsTransportError::local(HttpsTransportErrorReason::InvalidHeaderName))
    }

    pub(super) const fn maximum_bytes(self) -> usize {
        self.maximum_bytes
    }
}

/// A borrowed request whose sensitive body is copied only into zeroizing storage.
pub struct BoundedHttpsRequest<'a> {
    pub(super) method: HttpsMethod,
    pub(super) target: &'a str,
    pub(super) body: &'a [u8],
    pub(super) timeout: Duration,
    pub(super) limits: HttpsExchangeLimits,
    pub(super) content_type: Option<&'static str>,
    pub(super) accept: Option<&'static str>,
    pub(super) captured_response_header: Option<CapturedResponseHeader>,
}

impl std::fmt::Debug for BoundedHttpsRequest<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundedHttpsRequest")
            .field("method", &self.method)
            .field("target", &"[redacted]")
            .field(
                "body",
                &format_args!("[redacted; {} bytes]", self.body.len()),
            )
            .field("timeout", &self.timeout)
            .field("limits", &self.limits)
            .field("content_type", &self.content_type)
            .field("accept", &self.accept)
            .field("captured_response_header", &self.captured_response_header)
            .finish()
    }
}

impl<'a> BoundedHttpsRequest<'a> {
    /// Platform hard ceiling for an encoded relative request target.
    pub const MAXIMUM_TARGET_BYTES: usize = MAXIMUM_ENCODED_TARGET_BYTES;

    /// Validates the bounded request envelope.
    pub fn new(
        method: HttpsMethod,
        target: &'a str,
        body: &'a [u8],
        timeout: Duration,
        limits: HttpsExchangeLimits,
    ) -> Result<Self, HttpsTransportError> {
        validate_target(target)?;
        if timeout.is_zero() || timeout > MAXIMUM_TIMEOUT {
            return Err(HttpsTransportError::local(
                HttpsTransportErrorReason::InvalidTimeout,
            ));
        }
        if body.len() > limits.maximum_request_bytes() {
            return Err(HttpsTransportError::local(
                HttpsTransportErrorReason::RequestLimitExceeded,
            ));
        }
        Ok(Self {
            method,
            target,
            body,
            timeout,
            limits,
            content_type: None,
            accept: None,
            captured_response_header: None,
        })
    }

    /// Adds a validated `Content-Type` request header.
    pub fn with_content_type(
        mut self,
        content_type: &'static str,
    ) -> Result<Self, HttpsTransportError> {
        validate_header_value(content_type)?;
        self.content_type = Some(content_type);
        Ok(self)
    }

    /// Adds a validated `Accept` request header.
    pub fn with_accept(mut self, accept: &'static str) -> Result<Self, HttpsTransportError> {
        validate_header_value(accept)?;
        self.accept = Some(accept);
        Ok(self)
    }

    /// Selects one bounded response header for protocol-specific interpretation.
    #[must_use]
    pub const fn with_captured_response_header(mut self, header: CapturedResponseHeader) -> Self {
        self.captured_response_header = Some(header);
        self
    }
}

fn validate_header_value(value: &'static str) -> Result<(), HttpsTransportError> {
    if value.len() > MAXIMUM_CONTENT_NEGOTIATION_HEADER_BYTES {
        return Err(HttpsTransportError::local(
            HttpsTransportErrorReason::InvalidHeaderValue,
        ));
    }
    HeaderValue::from_str(value)
        .map(|_| ())
        .map_err(|_| HttpsTransportError::local(HttpsTransportErrorReason::InvalidHeaderValue))
}

const MAXIMUM_CONTENT_NEGOTIATION_HEADER_BYTES: usize = 256;

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
