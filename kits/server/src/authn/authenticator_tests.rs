// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::{Ready, ready};

use super::Authentication;
use crate::authn::{
    AuthenticatedPrincipal, AuthenticationDecision, AuthnError, AuthnErrorKind, Credentials,
    PrincipalId, PrincipalKind,
};

struct ExampleAuthentication;

impl Authentication for ExampleAuthentication {
    type Error = AuthnError;
    type AuthenticateFuture<'a> = Ready<Result<AuthenticationDecision, Self::Error>>;

    fn authenticate<'a>(&'a self, credentials: &'a Credentials) -> Self::AuthenticateFuture<'a> {
        match credentials {
            Credentials::Anonymous => ready(Ok(AuthenticationDecision::Anonymous)),
            Credentials::BearerToken(_) => {
                let principal_id = PrincipalId::new("user-1").expect("valid principal id");
                ready(Ok(AuthenticationDecision::Authenticated(
                    AuthenticatedPrincipal::new(principal_id, PrincipalKind::User),
                )))
            }
            Credentials::ApiKey(_) => ready(Ok(AuthenticationDecision::Rejected)),
            Credentials::ServiceToken(_) => {
                ready(Err(AuthnError::new(AuthnErrorKind::BackendUnavailable)))
            }
        }
    }
}

#[tokio::test]
async fn authn_rejection_is_distinct_from_infrastructure_error() {
    let authentication = ExampleAuthentication;

    let rejected = authentication
        .authenticate(&Credentials::ApiKey(crate::authn::ApiKey::new("api-key")))
        .await
        .expect("api key path should not fail infrastructurally");
    let infrastructure_error = authentication
        .authenticate(&Credentials::ServiceToken(crate::authn::ServiceToken::new(
            "service-token",
        )))
        .await
        .expect_err("service-token path should simulate infrastructure failure");

    assert_eq!(rejected, AuthenticationDecision::Rejected);
    assert_eq!(
        infrastructure_error.kind(),
        AuthnErrorKind::BackendUnavailable
    );
    assert_eq!(
        infrastructure_error.to_string(),
        "authentication infrastructure failed"
    );
}
