// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Generic authorization denial reasons.
///
/// This enum remains intentionally infrastructure-oriented. Product-specific
/// permissions, roles, and policy explanations belong in domain or service
/// crates, not in the shared server kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationDenyReason {
    /// Anonymous callers are not permitted for this authorization boundary.
    AnonymousNotAllowed,
    /// The principal lacks the required permission or scope.
    InsufficientPermissions,
    /// A generic policy evaluation denied the action.
    PolicyDenied,
}

/// Authorization outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationDecision {
    /// The action is permitted.
    Allow,
    /// The action is denied for the provided reason.
    Deny(AuthorizationDenyReason),
}

#[cfg(test)]
mod tests {
    use super::{AuthorizationDecision, AuthorizationDenyReason};

    #[test]
    fn deny_carries_reason() {
        let decision = AuthorizationDecision::Deny(AuthorizationDenyReason::PolicyDenied);

        assert_eq!(
            decision,
            AuthorizationDecision::Deny(AuthorizationDenyReason::PolicyDenied)
        );
    }
}
