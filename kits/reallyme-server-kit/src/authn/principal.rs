// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::{AuthnError, PrincipalIdValidationErrorReason};

const MAXIMUM_PRINCIPAL_ID_LENGTH: usize = 128;

/// Strongly typed principal identifier.
///
/// Principal IDs are intentionally not raw `String` values in public APIs.
/// This keeps authentication/authorization boundaries explicit and avoids
/// accidental mixing with unrelated string identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrincipalId(String);

impl PrincipalId {
    /// Constructs a validated principal identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, AuthnError> {
        let value = value.into();

        if value.is_empty() {
            return Err(AuthnError::invalid_principal_id(
                PrincipalIdValidationErrorReason::Empty,
            ));
        }

        if value.len() > MAXIMUM_PRINCIPAL_ID_LENGTH {
            return Err(AuthnError::invalid_principal_id(
                PrincipalIdValidationErrorReason::TooLong,
            ));
        }

        if value.bytes().all(is_allowed_principal_id_byte) {
            return Ok(Self(value));
        }

        Err(AuthnError::invalid_principal_id(
            PrincipalIdValidationErrorReason::InvalidCharacters,
        ))
    }

    /// Returns the validated principal identifier as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Generic authenticated principal kinds supported by the infrastructure
/// layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    /// End-user principal.
    User,
    /// Service or workload principal.
    Service,
}

/// Authenticated principal resolved by a successful authenticator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedPrincipal {
    principal_id: PrincipalId,
    kind: PrincipalKind,
}

impl AuthenticatedPrincipal {
    /// Creates an authenticated principal.
    pub const fn new(principal_id: PrincipalId, kind: PrincipalKind) -> Self {
        Self { principal_id, kind }
    }

    /// Returns the principal identifier.
    pub fn principal_id(&self) -> &PrincipalId {
        &self.principal_id
    }

    /// Returns the principal kind.
    pub const fn kind(&self) -> PrincipalKind {
        self.kind
    }
}

/// Explicit caller identity used at transport or service boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Principal {
    /// Explicit anonymous caller.
    Anonymous,
    /// Authenticated caller.
    Authenticated(AuthenticatedPrincipal),
}

impl Principal {
    /// Returns whether the principal is anonymous.
    pub const fn is_anonymous(&self) -> bool {
        matches!(self, Self::Anonymous)
    }

    /// Returns whether the principal is authenticated.
    pub const fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated(_))
    }

    /// Returns the authenticated principal, if present.
    pub const fn authenticated(&self) -> Option<&AuthenticatedPrincipal> {
        match self {
            Self::Anonymous => None,
            Self::Authenticated(principal) => Some(principal),
        }
    }
}

const fn is_allowed_principal_id_byte(value: u8) -> bool {
    value.is_ascii_alphanumeric()
        || value == b'-'
        || value == b'_'
        || value == b'.'
        || value == b':'
}

#[cfg(test)]
mod tests {
    use super::{AuthenticatedPrincipal, Principal, PrincipalId, PrincipalKind};
    use crate::authn::{AuthnError, AuthnErrorKind, PrincipalIdValidationErrorReason};

    #[test]
    fn principal_id_validation_is_typed() {
        assert_eq!(
            PrincipalId::new(""),
            Err(AuthnError::invalid_principal_id(
                PrincipalIdValidationErrorReason::Empty
            ))
        );
        assert_eq!(
            PrincipalId::new("bad id"),
            Err(AuthnError::invalid_principal_id(
                PrincipalIdValidationErrorReason::InvalidCharacters
            ))
        );
        assert_eq!(
            PrincipalId::new("a".repeat(129)),
            Err(AuthnError::invalid_principal_id(
                PrincipalIdValidationErrorReason::TooLong
            ))
        );
    }

    #[test]
    fn principal_model_distinguishes_anonymous_and_authenticated() {
        let principal_id = PrincipalId::new("service-api").expect("valid principal id");
        let authenticated = Principal::Authenticated(AuthenticatedPrincipal::new(
            principal_id,
            PrincipalKind::Service,
        ));

        assert!(Principal::Anonymous.is_anonymous());
        assert!(!Principal::Anonymous.is_authenticated());
        assert!(authenticated.is_authenticated());
        assert_eq!(
            authenticated
                .authenticated()
                .expect("authenticated principal should exist")
                .kind(),
            PrincipalKind::Service
        );
    }

    #[test]
    fn principal_id_errors_do_not_leak_raw_values() {
        let error = PrincipalId::new("bad id").expect_err("principal id should be invalid");

        assert_eq!(
            error.kind(),
            AuthnErrorKind::InvalidPrincipalId(PrincipalIdValidationErrorReason::InvalidCharacters)
        );
        assert_eq!(error.to_string(), "authentication infrastructure failed");
    }
}
