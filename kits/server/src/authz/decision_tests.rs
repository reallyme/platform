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
