// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Authentication boundaries and principal types.

mod authenticator;
mod credentials;
mod decision;
mod error;
mod principal;

pub use authenticator::Authentication;
pub use credentials::{ApiKey, BearerToken, Credentials, ServiceToken};
pub use decision::AuthenticationDecision;
pub use error::{AuthnError, AuthnErrorKind, PrincipalIdValidationErrorReason};
pub use principal::{AuthenticatedPrincipal, Principal, PrincipalId, PrincipalKind};
