// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Typed route-policy validation error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum HttpRoutePolicyError {
    /// HTTP listener name failed validation.
    #[error("http listener name failed validation")]
    ListenerName {
        /// Validation reason.
        reason: HttpRoutePolicyErrorReason,
    },
    /// Custom visibility class failed validation.
    #[error("http visibility class failed validation")]
    VisibilityClass {
        /// Validation reason.
        reason: HttpRoutePolicyErrorReason,
    },
    /// HTTP route prefix failed validation.
    #[error("http route prefix failed validation")]
    RoutePrefix {
        /// Validation reason.
        reason: HttpRoutePolicyErrorReason,
    },
    /// HTTP route rate-limit tier name failed validation.
    #[error("http rate-limit tier name failed validation")]
    RateLimitTier {
        /// Validation reason.
        reason: HttpRoutePolicyErrorReason,
    },
    /// HTTP route auth policy name failed validation.
    #[error("http auth policy name failed validation")]
    AuthPolicy {
        /// Validation reason.
        reason: HttpRoutePolicyErrorReason,
    },
}

/// Low-cardinality route-policy validation reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRoutePolicyErrorReason {
    /// Value is empty.
    Empty,
    /// Value is too long.
    TooLong,
    /// Value starts or ends with an invalid delimiter.
    InvalidBoundary,
    /// Value contains unsupported characters.
    InvalidCharacters,
    /// Route prefix is not a stable absolute path prefix.
    InvalidRouteShape,
    /// A route prefix was configured more than once.
    Duplicate,
}
