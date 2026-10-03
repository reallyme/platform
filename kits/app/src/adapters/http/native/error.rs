// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed failures for bounded HTTPS exchanges.

use thiserror::Error;

/// Whether a failed exchange is proven local or may have reached the peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpsDispatchOutcome {
    /// Validation or request construction failed before network dispatch.
    NotDispatched,
    /// Dispatch began, so the peer may have acted even without a response.
    RemoteOutcomeUnknown,
}

/// Stable, content-free classification for HTTPS transport failures.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum HttpsTransportErrorReason {
    /// The configured origin is not an absolute, authority-only HTTPS URL.
    #[error("the HTTPS origin is invalid")]
    InvalidOrigin,
    /// The relative request target is malformed or could change authority.
    #[error("the HTTPS request target is invalid")]
    InvalidTarget,
    /// A request or response byte limit is zero or exceeds the hard ceiling.
    #[error("an HTTPS exchange limit is invalid")]
    InvalidLimit,
    /// The request timeout is zero or exceeds the hard ceiling.
    #[error("the HTTPS request timeout is invalid")]
    InvalidTimeout,
    /// A configured HTTP header name is invalid.
    #[error("an HTTP header name is invalid")]
    InvalidHeaderName,
    /// A configured HTTP header value is invalid.
    #[error("an HTTP header value is invalid")]
    InvalidHeaderValue,
    /// The request body exceeds its configured byte limit.
    #[error("the HTTPS request body exceeds its byte limit")]
    RequestLimitExceeded,
    /// Memory for a sensitive request body could not be reserved.
    #[error("the HTTPS request body could not be allocated")]
    RequestAllocationFailed,
    /// The hardened HTTP client could not be initialized.
    #[error("the HTTPS client could not be initialized")]
    ClientInitializationFailed,
    /// The request could not be constructed.
    #[error("the HTTPS request could not be constructed")]
    RequestConstructionFailed,
    /// Dispatch or response-header receipt failed.
    #[error("the HTTPS exchange failed after dispatch began")]
    ExchangeFailed,
    /// The response body exceeds its configured byte limit.
    #[error("the HTTPS response body exceeds its byte limit")]
    ResponseLimitExceeded,
    /// A selected response header exceeds its configured byte limit.
    #[error("a selected HTTPS response header exceeds its byte limit")]
    ResponseHeaderLimitExceeded,
    /// Memory for sensitive response data could not be reserved.
    #[error("HTTPS response data could not be allocated")]
    ResponseAllocationFailed,
    /// The response body could not be read to completion.
    #[error("the HTTPS response body could not be read")]
    ResponseReadFailed,
}

/// A bounded HTTPS failure with explicit remote-outcome certainty.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[error("{reason}")]
pub struct HttpsTransportError {
    reason: HttpsTransportErrorReason,
    dispatch_outcome: HttpsDispatchOutcome,
}

impl HttpsTransportError {
    pub(super) const fn local(reason: HttpsTransportErrorReason) -> Self {
        Self {
            reason,
            dispatch_outcome: HttpsDispatchOutcome::NotDispatched,
        }
    }

    pub(super) const fn dispatched(reason: HttpsTransportErrorReason) -> Self {
        Self {
            reason,
            dispatch_outcome: HttpsDispatchOutcome::RemoteOutcomeUnknown,
        }
    }

    /// Returns the stable, content-free failure classification.
    #[must_use]
    pub const fn reason(&self) -> HttpsTransportErrorReason {
        self.reason
    }

    /// Returns whether the peer may have observed the request.
    #[must_use]
    pub const fn dispatch_outcome(&self) -> HttpsDispatchOutcome {
        self.dispatch_outcome
    }
}
