// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
#[must_use = "authorization decisions must be enforced or explicitly discarded"]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationDecision {
    /// The action is permitted.
    Allow,
    /// The action is denied for the provided reason.
    Deny(AuthorizationDenyReason),
}

/// Typed result of enforcing a denied authorization decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("authorization denied")]
pub struct AuthorizationDenied {
    reason: AuthorizationDenyReason,
}

impl AuthorizationDenied {
    /// Returns the safe, low-cardinality denial reason.
    pub const fn reason(self) -> AuthorizationDenyReason {
        self.reason
    }
}

impl AuthorizationDecision {
    /// Enforces the decision at an authorization boundary.
    pub const fn require_allow(self) -> Result<(), AuthorizationDenied> {
        match self {
            Self::Allow => Ok(()),
            Self::Deny(reason) => Err(AuthorizationDenied { reason }),
        }
    }
}

#[cfg(test)]
#[path = "decision_tests.rs"]
mod tests;
