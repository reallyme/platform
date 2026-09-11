// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Why a principal identifier is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalIdValidationErrorReason {
    /// The value was empty.
    Empty,
    /// The value exceeded the supported maximum length.
    TooLong,
    /// The value contained unsupported characters.
    InvalidCharacters,
}

/// Authentication infrastructure failure kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthnErrorKind {
    /// The backend dependency required for authentication is unavailable.
    BackendUnavailable,
    /// The authentication runtime is misconfigured.
    InvalidConfiguration,
    /// The principal identifier is invalid.
    InvalidPrincipalId(PrincipalIdValidationErrorReason),
    /// Internal infrastructure failure.
    Internal,
}

/// Typed authentication infrastructure failures.
///
/// These failures are distinct from ordinary credential rejection. Transport
/// layers should map them intentionally and should not leak internal
/// diagnostics or credential details publicly.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("authentication infrastructure failed")]
pub struct AuthnError {
    kind: AuthnErrorKind,
}

impl AuthnError {
    /// Creates an authentication infrastructure failure.
    pub const fn new(kind: AuthnErrorKind) -> Self {
        Self { kind }
    }

    /// Creates a principal-ID validation failure.
    pub const fn invalid_principal_id(reason: PrincipalIdValidationErrorReason) -> Self {
        Self {
            kind: AuthnErrorKind::InvalidPrincipalId(reason),
        }
    }

    /// Returns the low-cardinality failure kind.
    pub const fn kind(self) -> AuthnErrorKind {
        self.kind
    }
}
