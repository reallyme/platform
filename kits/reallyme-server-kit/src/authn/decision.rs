// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::principal::{AuthenticatedPrincipal, Principal};

/// Result of an authentication attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthenticationDecision {
    /// The credentials were accepted and an authenticated principal was
    /// produced.
    Authenticated(AuthenticatedPrincipal),
    /// The request resolved explicitly as anonymous.
    Anonymous,
    /// The credentials were rejected without an infrastructure failure.
    Rejected,
}

impl AuthenticationDecision {
    /// Converts a successful authentication outcome into a generic principal.
    pub fn into_principal(self) -> Option<Principal> {
        match self {
            Self::Authenticated(principal) => Some(Principal::Authenticated(principal)),
            Self::Anonymous => Some(Principal::Anonymous),
            Self::Rejected => None,
        }
    }
}
