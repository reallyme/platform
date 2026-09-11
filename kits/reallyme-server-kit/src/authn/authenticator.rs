// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::future::Future;
use std::error::Error as StdError;

use super::credentials::Credentials;
use super::decision::AuthenticationDecision;

/// Authentication boundary for transport adapters.
///
/// The trait is asynchronous without relying on macro-based async traits. This
/// keeps the public contract explicit and avoids hidden allocation decisions.
///
/// This trait is intended for static dispatch through concrete application
/// state. It is deliberately not object-safe because the generic associated
/// future lets each implementation choose its own future type without boxing.
/// If a service needs runtime selection between authenticators, it should wrap
/// that choice in a concrete enum or adapter that implements this trait, rather
/// than erasing errors or adding implicit allocation at the boundary.
///
/// `Send + Sync` is required because authenticators are expected to live in
/// shared application state in typical service setups.
///
/// # Examples
///
/// ```rust
/// use std::future::{Ready, ready};
///
/// use reallyme_server_kit::authn::{
///     AuthenticatedPrincipal, Authentication, AuthenticationDecision, AuthnError,
///     AuthnErrorKind, BearerToken, Credentials, PrincipalId, PrincipalKind,
/// };
///
/// struct ExampleAuthenticator;
///
/// impl Authentication for ExampleAuthenticator {
///     type Error = AuthnError;
///     type AuthenticateFuture<'a> = Ready<Result<AuthenticationDecision, Self::Error>>;
///
///     fn authenticate<'a>(&'a self, credentials: &'a Credentials) -> Self::AuthenticateFuture<'a> {
///         match credentials {
///             Credentials::BearerToken(_) => {
///                 let principal_id = PrincipalId::new("user-42").expect("valid principal id");
///                 ready(Ok(AuthenticationDecision::Authenticated(
///                     AuthenticatedPrincipal::new(principal_id, PrincipalKind::User),
///                 )))
///             }
///             Credentials::Anonymous => ready(Ok(AuthenticationDecision::Anonymous)),
///             Credentials::ApiKey(_) | Credentials::ServiceToken(_) => {
///                 ready(Ok(AuthenticationDecision::Rejected))
///             }
///         }
///     }
/// }
///
/// let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
/// let authenticator = ExampleAuthenticator;
/// let credentials = Credentials::BearerToken(BearerToken::new("valid-token"));
///
/// let decision = runtime
///     .block_on(async { authenticator.authenticate(&credentials).await })
///     .expect("authentication should not fail infrastructurally");
///
/// assert!(matches!(
///     decision,
///     AuthenticationDecision::Authenticated(ref principal)
///         if principal.principal_id().as_str() == "user-42"
/// ));
/// ```
pub trait Authentication: Send + Sync {
    /// Infrastructure failure emitted by the authenticator.
    ///
    /// The bound intentionally requires a real typed error rather than
    /// allowing opaque placeholders such as `()`, `String`, or `&'static str`.
    /// Implementations should expose a documented enum or other structured
    /// error type that can be inspected and mapped explicitly at transport
    /// boundaries.
    type Error: StdError + Send + Sync + 'static;
    /// Future returned by the authentication operation.
    type AuthenticateFuture<'a>: Future<Output = Result<AuthenticationDecision, Self::Error>>
        + Send
        + 'a
    where
        Self: 'a,
        Credentials: 'a;

    /// Attempts to authenticate the provided credentials.
    fn authenticate<'a>(&'a self, credentials: &'a Credentials) -> Self::AuthenticateFuture<'a>;
}

#[cfg(test)]
mod tests {
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

        fn authenticate<'a>(
            &'a self,
            credentials: &'a Credentials,
        ) -> Self::AuthenticateFuture<'a> {
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
}
