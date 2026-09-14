// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Why a permission name is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionValidationErrorReason {
    /// The value was empty.
    Empty,
    /// The value exceeded the supported maximum length.
    TooLong,
    /// The value contained unsupported characters.
    InvalidCharacters,
}

/// Authorization infrastructure failure kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthzErrorKind {
    /// The backend dependency required for authorization is unavailable.
    BackendUnavailable,
    /// The authorization runtime is misconfigured.
    InvalidConfiguration,
    /// The permission name is invalid.
    InvalidPermission(PermissionValidationErrorReason),
    /// Internal infrastructure failure.
    Internal,
}

/// Typed authorization infrastructure failures.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("authorization infrastructure failed")]
pub struct AuthzError {
    kind: AuthzErrorKind,
}

impl AuthzError {
    /// Creates an authorization infrastructure failure.
    pub const fn new(kind: AuthzErrorKind) -> Self {
        Self { kind }
    }

    /// Creates a permission-name validation failure.
    pub const fn invalid_permission(reason: PermissionValidationErrorReason) -> Self {
        Self {
            kind: AuthzErrorKind::InvalidPermission(reason),
        }
    }

    /// Returns the low-cardinality failure kind.
    pub const fn kind(self) -> AuthzErrorKind {
        self.kind
    }
}
