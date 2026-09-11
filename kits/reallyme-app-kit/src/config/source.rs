// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

const MAX_CONFIG_SOURCE_NAME_BYTES: usize = 128;

/// App config source format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppConfigFormat {
    /// JSON with comments.
    Jsonc,
    /// Environment variables.
    Environment,
    /// Host-provided structured config object.
    HostObject,
}

/// Standard app config profile.
///
/// App crates commonly ship `config/local.jsonc`, `config/staging.jsonc`, and
/// `config/prod.jsonc`. Hosts choose the profile; app cores never inspect the
/// process environment or filesystem directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppConfigProfile {
    /// Local developer profile.
    Local,
    /// Staging/pre-production profile.
    Staging,
    /// Production profile.
    Prod,
}

impl AppConfigProfile {
    /// Returns the conventional JSONC file name for this profile.
    pub const fn jsonc_file_name(self) -> &'static str {
        match self {
            Self::Local => "local.jsonc",
            Self::Staging => "staging.jsonc",
            Self::Prod => "prod.jsonc",
        }
    }
}

/// Safe descriptor for where app config came from.
///
/// This type must never contain raw config bodies or secret values. It is only
/// for startup summaries and diagnostics such as `local.jsonc` or `env`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfigSource {
    format: AppConfigFormat,
    name: String,
}

impl AppConfigSource {
    /// Constructs a safe app config source descriptor.
    pub fn new(format: AppConfigFormat, name: impl Into<String>) -> Result<Self, AppKitError> {
        let name = name.into();

        if name.is_empty() {
            return Err(AppKitError::new(
                AppKitField::ConfigSource,
                AppKitErrorReason::Empty,
            ));
        }

        if name.len() > MAX_CONFIG_SOURCE_NAME_BYTES {
            return Err(AppKitError::new(
                AppKitField::ConfigSource,
                AppKitErrorReason::TooLong,
            ));
        }

        if name.chars().any(char::is_whitespace) {
            return Err(AppKitError::new(
                AppKitField::ConfigSource,
                AppKitErrorReason::InvalidCharacter,
            ));
        }

        Ok(Self { format, name })
    }

    /// Returns the source format.
    pub const fn format(&self) -> AppConfigFormat {
        self.format
    }

    /// Returns the safe source name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl fmt::Debug for AppConfigSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppConfigSource")
            .field("format", &self.format)
            .field("name", &self.name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfigFormat, AppConfigProfile, AppConfigSource};
    use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

    #[test]
    fn accepts_safe_config_source_names() {
        let source =
            AppConfigSource::new(AppConfigFormat::Jsonc, "local.jsonc").expect("valid source name");

        assert_eq!(source.name(), "local.jsonc");
    }

    #[test]
    fn rejects_config_source_with_whitespace() {
        assert_eq!(
            AppConfigSource::new(AppConfigFormat::Jsonc, "local config.jsonc"),
            Err(AppKitError::new(
                AppKitField::ConfigSource,
                AppKitErrorReason::InvalidCharacter,
            ))
        );
    }

    #[test]
    fn profile_file_names_are_stable() {
        assert_eq!(AppConfigProfile::Local.jsonc_file_name(), "local.jsonc");
        assert_eq!(AppConfigProfile::Staging.jsonc_file_name(), "staging.jsonc");
        assert_eq!(AppConfigProfile::Prod.jsonc_file_name(), "prod.jsonc");
    }
}
