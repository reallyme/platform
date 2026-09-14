// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
