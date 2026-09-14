// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{AppPermission, AppPermissionName};
use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

#[test]
fn accepts_symbolic_permission_names() {
    let name = AppPermissionName::new("handles.precheck").expect("valid permission name fixture");
    let permission = AppPermission::new(name);

    assert_eq!(permission.name().as_str(), "handles.precheck");
}

#[test]
fn rejects_permission_names_with_whitespace() {
    assert_eq!(
        AppPermissionName::new("handles precheck"),
        Err(AppKitError::new(
            AppKitField::PermissionName,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
}

#[test]
fn rejects_permission_names_with_repeated_symbol_separators() {
    assert_eq!(
        AppPermissionName::new("handles..precheck"),
        Err(AppKitError::new(
            AppKitField::PermissionName,
            AppKitErrorReason::InvalidCharacter,
        )),
    );
    assert_eq!(
        AppPermissionName::new("handles__precheck"),
        Err(AppKitError::new(
            AppKitField::PermissionName,
            AppKitErrorReason::InvalidCharacter,
        )),
    );
    assert_eq!(
        AppPermissionName::new("handles--precheck"),
        Err(AppKitError::new(
            AppKitField::PermissionName,
            AppKitErrorReason::InvalidCharacter,
        )),
    );
}
