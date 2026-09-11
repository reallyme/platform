// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{AppKitError, AppKitField};
use crate::name_validator::{NameValidation, validate_name};

const MAX_APP_NAME_BYTES: usize = 63;

/// Validated logical app name.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AppName(String);

impl AppName {
    /// Constructs a validated app name.
    pub fn new(value: impl Into<String>) -> Result<Self, AppKitError> {
        let value = value.into();
        validate_dns_label(value.as_str(), AppKitField::AppName)?;

        Ok(Self(value))
    }

    /// Returns the app name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("AppName").field(&self.0).finish()
    }
}

impl Serialize for AppName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

pub(crate) fn validate_dns_label(value: &str, field: AppKitField) -> Result<(), AppKitError> {
    let rules = NameValidation::new(b"-", b"-", MAX_APP_NAME_BYTES, false);
    validate_name(value, field, &rules)
}

#[cfg(test)]
mod tests {
    use super::AppName;
    use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

    #[test]
    fn validates_app_names() {
        let name = AppName::new("reallyme-api").expect("valid app name");

        assert_eq!(name.as_str(), "reallyme-api");
    }

    #[test]
    fn rejects_invalid_app_names() {
        assert_eq!(
            AppName::new("ReallyMe API"),
            Err(AppKitError::new(
                AppKitField::AppName,
                AppKitErrorReason::InvalidCharacter,
            ))
        );
    }
}
