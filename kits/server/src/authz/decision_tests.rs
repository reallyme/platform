// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{AuthorizationDecision, AuthorizationDenyReason};

#[test]
fn deny_carries_reason() {
    let decision = AuthorizationDecision::Deny(AuthorizationDenyReason::PolicyDenied);

    assert_eq!(
        decision,
        AuthorizationDecision::Deny(AuthorizationDenyReason::PolicyDenied)
    );
}

#[test]
fn require_allow_rejects_denial_with_typed_reason() {
    assert_eq!(AuthorizationDecision::Allow.require_allow(), Ok(()));
    let denied = AuthorizationDecision::Deny(AuthorizationDenyReason::PolicyDenied)
        .require_allow()
        .expect_err("denial must not be ignored");
    assert_eq!(denied.reason(), AuthorizationDenyReason::PolicyDenied);
}
