// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::{Extensions, HeaderMap, HeaderName, HeaderValue};
use thiserror::Error;

use super::{
    IdentifierHeaderError, RequestId, TraceId, X_REQUEST_ID, X_TRACE_ID,
    request_id_from_extensions, trace_id_from_extensions,
};

/// Standard header used to identify the internal service caller.
pub const X_INTERNAL_CALLER: HeaderName = HeaderName::from_static("x-internal-caller");
/// Standard header used to carry shared internal service tokens.
pub const X_SERVICE_TOKEN: HeaderName = HeaderName::from_static("x-service-token");

/// Host-verified transport security posture attached to a request.
///
/// The host is responsible for setting this extension only after it has
/// independently verified the claimed transport property. App code must never
/// derive this from raw user-controlled headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedTransportSecurity {
    mutual_tls_verified: bool,
}

impl VerifiedTransportSecurity {
    /// Marks a request as having passed verified mutual-TLS client-auth.
    pub const fn mutual_tls_verified() -> Self {
        Self {
            mutual_tls_verified: true,
        }
    }

    /// Marks a request as having no verified client-certificate proof.
    pub const fn not_verified() -> Self {
        Self {
            mutual_tls_verified: false,
        }
    }

    /// Returns whether mutual-TLS client-auth was verified by the host.
    pub const fn is_mutual_tls_verified(self) -> bool {
        self.mutual_tls_verified
    }
}

/// Correlation identifiers propagated on an internal service call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternalCallCorrelationIds {
    request_id: RequestId,
    trace_id: TraceId,
}

impl InternalCallCorrelationIds {
    /// Returns the propagated request ID.
    pub const fn request_id(self) -> RequestId {
        self.request_id
    }

    /// Returns the propagated trace ID.
    pub const fn trace_id(self) -> TraceId {
        self.trace_id
    }
}

/// Low-cardinality internal header propagation error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum InternalRequestHeaderError {
    /// The internal caller identifier could not be represented safely.
    #[error("internal caller header is invalid")]
    InvalidCaller,
    /// The shared service token could not be represented safely.
    #[error("internal service token header is invalid")]
    InvalidServiceToken,
    /// Request or trace ID propagation failed.
    #[error("internal correlation header is invalid")]
    InvalidCorrelationId,
}

/// Returns correlation IDs for an internal call, generating missing values.
pub fn internal_call_correlation_ids_from_extensions(
    extensions: &Extensions,
) -> InternalCallCorrelationIds {
    InternalCallCorrelationIds {
        request_id: request_id_from_extensions(extensions).unwrap_or_else(RequestId::generate),
        trace_id: trace_id_from_extensions(extensions).unwrap_or_else(TraceId::generate),
    }
}

/// Attaches internal routing/auth and correlation headers for a private call.
///
/// This helper intentionally never logs or exposes raw service tokens. It
/// reuses normalized request/trace identifiers when the inbound request already
/// carries them and generates fresh values otherwise.
pub fn attach_internal_request_headers(
    headers: &mut HeaderMap,
    extensions: &Extensions,
    caller: &str,
    service_token: Option<&str>,
) -> Result<InternalCallCorrelationIds, InternalRequestHeaderError> {
    let correlation_ids = internal_call_correlation_ids_from_extensions(extensions);

    headers.insert(
        X_INTERNAL_CALLER,
        HeaderValue::from_str(caller).map_err(|_| InternalRequestHeaderError::InvalidCaller)?,
    );
    headers.insert(
        X_REQUEST_ID,
        correlation_ids
            .request_id()
            .to_header_value()
            .map_err(map_identifier_error)?,
    );
    headers.insert(
        X_TRACE_ID,
        correlation_ids
            .trace_id()
            .to_header_value()
            .map_err(map_identifier_error)?,
    );

    if let Some(service_token) = service_token {
        headers.insert(
            X_SERVICE_TOKEN,
            HeaderValue::from_str(service_token)
                .map_err(|_| InternalRequestHeaderError::InvalidServiceToken)?,
        );
    }

    Ok(correlation_ids)
}

fn map_identifier_error(_error: IdentifierHeaderError) -> InternalRequestHeaderError {
    InternalRequestHeaderError::InvalidCorrelationId
}

#[cfg(test)]
#[path = "internal_tests.rs"]
mod tests;
