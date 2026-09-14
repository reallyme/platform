// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::future::Future;
use std::error::Error as StdError;

use crate::authn::Principal;

use super::decision::AuthorizationDecision;
use super::permission::Permission;

/// Authorization boundary for transport adapters and service entrypoints.
///
/// Like authentication, this trait uses a generic associated future instead of
/// `async_trait`. That makes the cost model explicit and keeps typed errors
/// intact, but it also means the trait is intended for static dispatch through
/// concrete application state rather than direct `dyn Authorization` usage.
/// Services that need runtime policy selection should model that selection as
/// a concrete enum or adapter with one typed error surface.
///
/// # Examples
///
/// ```rust
/// use std::future::{Ready, ready};
///
/// use reallyme_server_kit::authn::{AuthenticatedPrincipal, Principal, PrincipalId, PrincipalKind};
/// use reallyme_server_kit::authz::{
///     Authorization, AuthorizationDecision, AuthorizationDenyReason, AuthzError, NamedPermission,
///     Permission, PermissionName,
/// };
///
/// struct ExampleAuthorizer;
///
/// impl Authorization<NamedPermission, ()> for ExampleAuthorizer {
///     type Error = AuthzError;
///     type AuthorizeFuture<'a> = Ready<Result<AuthorizationDecision, Self::Error>>;
///
///     fn authorize<'a>(
///         &'a self,
///         principal: &'a Principal,
///         permission: &'a NamedPermission,
///         _resource: &'a (),
///     ) -> Self::AuthorizeFuture<'a> {
///         match principal {
///             Principal::Authenticated(authenticated)
///                 if authenticated.kind() == PrincipalKind::Service
///                     && permission.permission_name().as_str() == "health.read" =>
///             {
///                 ready(Ok(AuthorizationDecision::Allow))
///             }
///             Principal::Anonymous => ready(Ok(AuthorizationDecision::Deny(
///                 AuthorizationDenyReason::AnonymousNotAllowed,
///             ))),
///             Principal::Authenticated(_) => ready(Ok(AuthorizationDecision::Deny(
///                 AuthorizationDenyReason::InsufficientPermissions,
///             ))),
///         }
///     }
/// }
///
/// let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
/// let authorizer = ExampleAuthorizer;
/// let principal_id = PrincipalId::new("service-api").expect("valid principal id");
/// let principal = Principal::Authenticated(AuthenticatedPrincipal::new(
///     principal_id,
///     PrincipalKind::Service,
/// ));
/// let permission = NamedPermission::new(
///     PermissionName::new("health.read").expect("valid permission name"),
/// );
///
/// let decision = runtime
///     .block_on(async { authorizer.authorize(&principal, &permission, &()).await })
///     .expect("authorization should not fail infrastructurally");
///
/// assert_eq!(decision, AuthorizationDecision::Allow);
/// ```
pub trait Authorization<RequestedPermission, Resource>: Send + Sync
where
    RequestedPermission: Permission,
{
    /// Infrastructure failure emitted by the authorizer.
    type Error: StdError + Send + Sync + 'static;
    /// Future returned by the authorization operation.
    type AuthorizeFuture<'a>: Future<Output = Result<AuthorizationDecision, Self::Error>>
        + Send
        + 'a
    where
        Self: 'a,
        RequestedPermission: 'a,
        Resource: 'a;

    /// Evaluates whether the permission on the resource is permitted.
    fn authorize<'a>(
        &'a self,
        principal: &'a Principal,
        permission: &'a RequestedPermission,
        resource: &'a Resource,
    ) -> Self::AuthorizeFuture<'a>;
}

#[cfg(test)]
#[path = "authorizer_tests.rs"]
mod tests;
