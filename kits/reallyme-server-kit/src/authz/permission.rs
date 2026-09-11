// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::{AuthzError, PermissionValidationErrorReason};

const MAXIMUM_PERMISSION_NAME_LENGTH: usize = 128;

/// Strongly typed permission name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermissionName(String);

impl PermissionName {
    /// Constructs a validated permission name.
    pub fn new(value: impl Into<String>) -> Result<Self, AuthzError> {
        let value = value.into();

        if value.is_empty() {
            return Err(AuthzError::invalid_permission(
                PermissionValidationErrorReason::Empty,
            ));
        }

        if value.len() > MAXIMUM_PERMISSION_NAME_LENGTH {
            return Err(AuthzError::invalid_permission(
                PermissionValidationErrorReason::TooLong,
            ));
        }

        if value.bytes().all(is_allowed_permission_name_byte) {
            return Ok(Self(value));
        }

        Err(AuthzError::invalid_permission(
            PermissionValidationErrorReason::InvalidCharacters,
        ))
    }

    /// Returns the validated permission name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Permission descriptor trait.
///
/// Service or domain crates should define their own permission enums or
/// newtypes and implement this trait rather than passing raw strings through
/// authorization boundaries.
pub trait Permission: Send + Sync {
    /// Returns the stable permission name.
    fn permission_name(&self) -> &PermissionName;
}

/// Simple named permission helper for callers that do not need a richer type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedPermission {
    permission_name: PermissionName,
}

impl NamedPermission {
    /// Creates a named permission helper.
    pub const fn new(permission_name: PermissionName) -> Self {
        Self { permission_name }
    }
}

impl Permission for NamedPermission {
    fn permission_name(&self) -> &PermissionName {
        &self.permission_name
    }
}

const fn is_allowed_permission_name_byte(value: u8) -> bool {
    value.is_ascii_alphanumeric()
        || value == b'-'
        || value == b'_'
        || value == b'.'
        || value == b':'
}

#[cfg(test)]
mod tests {
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
}
