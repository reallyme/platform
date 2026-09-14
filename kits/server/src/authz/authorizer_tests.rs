// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::{Ready, ready};

use super::Authorization;
use crate::authn::{AuthenticatedPrincipal, Principal, PrincipalId, PrincipalKind};
use crate::authz::{
    AuthorizationDecision, AuthorizationDenyReason, AuthzError, NamedPermission, Permission,
    PermissionName,
};

struct ExampleAuthorization;

impl Authorization<NamedPermission, ()> for ExampleAuthorization {
    type Error = AuthzError;
    type AuthorizeFuture<'a> = Ready<Result<AuthorizationDecision, Self::Error>>;

    fn authorize<'a>(
        &'a self,
        principal: &'a Principal,
        permission: &'a NamedPermission,
        _resource: &'a (),
    ) -> Self::AuthorizeFuture<'a> {
        match principal {
            Principal::Anonymous => ready(Ok(AuthorizationDecision::Deny(
                AuthorizationDenyReason::AnonymousNotAllowed,
            ))),
            Principal::Authenticated(authenticated)
                if authenticated.kind() == PrincipalKind::Service
                    && permission.permission_name().as_str() == "health.read" =>
            {
                ready(Ok(AuthorizationDecision::Allow))
            }
            Principal::Authenticated(_) => ready(Ok(AuthorizationDecision::Deny(
                AuthorizationDenyReason::InsufficientPermissions,
            ))),
        }
    }
}

#[tokio::test]
async fn authz_allow_and_deny_behavior_is_typed() {
    let authorizer = ExampleAuthorization;
    let service_principal = Principal::Authenticated(AuthenticatedPrincipal::new(
        PrincipalId::new("service-api").expect("valid principal id"),
        PrincipalKind::Service,
    ));
    let user_principal = Principal::Authenticated(AuthenticatedPrincipal::new(
        PrincipalId::new("user-1").expect("valid principal id"),
        PrincipalKind::User,
    ));
    let permission =
        NamedPermission::new(PermissionName::new("health.read").expect("valid permission"));

    let allowed = authorizer
        .authorize(&service_principal, &permission, &())
        .await
        .expect("authorization should not fail infrastructurally");
    let denied = authorizer
        .authorize(&user_principal, &permission, &())
        .await
        .expect("authorization should not fail infrastructurally");
    let anonymous = authorizer
        .authorize(&Principal::Anonymous, &permission, &())
        .await
        .expect("authorization should not fail infrastructurally");

    assert_eq!(allowed, AuthorizationDecision::Allow);
    assert_eq!(
        denied,
        AuthorizationDecision::Deny(AuthorizationDenyReason::InsufficientPermissions)
    );
    assert_eq!(
        anonymous,
        AuthorizationDecision::Deny(AuthorizationDenyReason::AnonymousNotAllowed)
    );
}
