// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Authorization boundaries and decision types.

mod authorizer;
mod decision;
mod error;
mod permission;

pub use authorizer::Authorization;
pub use decision::{AuthorizationDecision, AuthorizationDenyReason};
pub use error::{AuthzError, AuthzErrorKind, PermissionValidationErrorReason};
pub use permission::{NamedPermission, Permission, PermissionName};
