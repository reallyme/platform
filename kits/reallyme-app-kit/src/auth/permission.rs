// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{AppKitError, AppKitField};
use crate::name_validator::{NameValidation, validate_name};

const MAX_PERMISSION_NAME_BYTES: usize = 128;

/// Validated host-neutral app permission name.
///
/// App-kit intentionally does not define product permissions. App crates and
/// domain crates own concrete permission catalogs; this type only gives those
/// catalogs a stable, low-cardinality, non-stringly representation that can be
/// shared by host adapters.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AppPermissionName(String);

impl AppPermissionName {
    /// Constructs a validated app permission name.
    pub fn new(value: impl Into<String>) -> Result<Self, AppKitError> {
        let value = value.into();
        validate_symbolic_name(
            value.as_str(),
            AppKitField::PermissionName,
            MAX_PERMISSION_NAME_BYTES,
        )?;

        Ok(Self(value))
    }

    /// Returns the validated permission name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppPermissionName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AppPermissionName")
            .field(&self.0)
            .finish()
    }
}

impl Serialize for AppPermissionName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppPermissionName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Host-neutral app permission.
///
/// This is a generic wrapper, not an authorization policy. Concrete policy
/// decisions stay in app crates or domain crates so app-kit can remain reusable
/// across server and Worker hosts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppPermission {
    name: AppPermissionName,
}

impl AppPermission {
    /// Constructs a permission from a validated permission name.
    pub const fn new(name: AppPermissionName) -> Self {
        Self { name }
    }

    /// Returns the permission name.
    pub const fn name(&self) -> &AppPermissionName {
        &self.name
    }
}

pub(crate) fn validate_symbolic_name(
    value: &str,
    field: AppKitField,
    max_len: usize,
) -> Result<(), AppKitError> {
    let rules = NameValidation::new(b"-_.", b"-_.", max_len, true);
    validate_name(value, field, &rules)
}

#[cfg(test)]
mod tests {
    use super::{AppPermission, AppPermissionName};
    use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

    #[test]
    fn accepts_symbolic_permission_names() {
        let name =
            AppPermissionName::new("handles.precheck").expect("valid permission name fixture");
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
}
