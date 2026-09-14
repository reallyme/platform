// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{NamedPermission, Permission, PermissionName};
use crate::authz::{AuthzError, PermissionValidationErrorReason};

#[test]
fn permission_name_validation_is_typed() {
    assert_eq!(
        PermissionName::new(""),
        Err(AuthzError::invalid_permission(
            PermissionValidationErrorReason::Empty
        ))
    );
    assert_eq!(
        PermissionName::new("bad permission"),
        Err(AuthzError::invalid_permission(
            PermissionValidationErrorReason::InvalidCharacters
        ))
    );
    assert_eq!(
        PermissionName::new("a".repeat(129)),
        Err(AuthzError::invalid_permission(
            PermissionValidationErrorReason::TooLong
        ))
    );
}

#[test]
fn named_permission_exposes_typed_permission_name() {
    let permission_name =
        PermissionName::new("health.read").expect("permission name fixture should be valid");
    let permission = NamedPermission::new(permission_name);

    assert_eq!(permission.permission_name().as_str(), "health.read");
}
