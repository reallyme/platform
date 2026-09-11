// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::StartupError;

/// Validated server-process identity used in logs, metrics labels, and other
/// cross-cutting infrastructure surfaces.
///
/// The validation rules intentionally mirror a conservative DNS-label style
/// subset to avoid invalid or surprising values propagating into operational
/// systems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerName(String);

impl ServerName {
    /// Constructs a validated server name.
    pub fn new(value: impl Into<String>) -> Result<Self, StartupError> {
        let value = value.into();

        if value.is_empty() {
            return Err(StartupError::EmptyServerName);
        }

        if value.len() > 63 {
            return Err(StartupError::ServerNameTooLong);
        }

        if value.starts_with('-') || value.ends_with('-') {
            return Err(StartupError::InvalidServerNameBoundary);
        }

        if value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Ok(Self(value));
        }

        Err(StartupError::InvalidServerName)
    }

    /// Returns the validated server name as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
