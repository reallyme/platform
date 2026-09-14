// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

const MAX_APP_VERSION_BYTES: usize = 64;

/// Validated app version string.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AppVersion(String);

impl AppVersion {
    /// Constructs a validated app version.
    pub fn new(value: impl Into<String>) -> Result<Self, AppKitError> {
        let value = value.into();

        if value.is_empty() {
            return Err(AppKitError::new(
                AppKitField::AppVersion,
                AppKitErrorReason::Empty,
            ));
        }

        if value.len() > MAX_APP_VERSION_BYTES {
            return Err(AppKitError::new(
                AppKitField::AppVersion,
                AppKitErrorReason::TooLong,
            ));
        }

        if value.chars().any(char::is_whitespace) {
            return Err(AppKitError::new(
                AppKitField::AppVersion,
                AppKitErrorReason::InvalidCharacter,
            ));
        }

        validate_semver(value.as_str())?;

        Ok(Self(value))
    }

    /// Returns the app version.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("AppVersion").field(&self.0).finish()
    }
}

impl Serialize for AppVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

fn validate_semver(value: &str) -> Result<(), AppKitError> {
    if value.contains('+') {
        return Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ));
    }

    let mut prerelease_parts = value.splitn(2, '-');
    let core = prerelease_parts.next().ok_or_else(|| {
        AppKitError::new(AppKitField::AppVersion, AppKitErrorReason::InvalidCharacter)
    })?;
    let prerelease = prerelease_parts.next();

    validate_semver_core(core)?;
    if let Some(prerelease) = prerelease {
        validate_semver_identifiers(prerelease)?;
    }

    Ok(())
}

fn validate_semver_identifiers(value: &str) -> Result<(), AppKitError> {
    if value.is_empty() {
        return Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ));
    }

    for identifier in value.split('.') {
        if identifier.is_empty() {
            return Err(AppKitError::new(
                AppKitField::AppVersion,
                AppKitErrorReason::InvalidCharacter,
            ));
        }
        if identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            if identifier.bytes().all(|byte| byte.is_ascii_digit())
                && identifier.len() > 1
                && identifier.as_bytes()[0] == b'0'
            {
                return Err(AppKitError::new(
                    AppKitField::AppVersion,
                    AppKitErrorReason::InvalidCharacter,
                ));
            }

            continue;
        }

        return Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ));
    }

    Ok(())
}

fn validate_semver_core(core: &str) -> Result<(), AppKitError> {
    let mut segments = core.split('.');
    let mut count = 0usize;

    for segment in segments.by_ref() {
        count = count.saturating_add(1);
        validate_semver_identifier(segment)?;
    }

    if count != 3 {
        return Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ));
    }

    Ok(())
}

fn validate_semver_identifier(segment: &str) -> Result<(), AppKitError> {
    if segment.is_empty() || segment.len() > 19 {
        return Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ));
    }

    if !segment.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ));
    }

    if segment.len() > 1 && segment.as_bytes()[0] == b'0' {
        return Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ));
    }

    Ok(())
}

#[cfg(test)]
#[path = "app_version_tests.rs"]
mod tests;
