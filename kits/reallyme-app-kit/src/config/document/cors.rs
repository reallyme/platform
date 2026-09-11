// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::AppConfigDocumentError;
use super::raw::RawCorsConfig;
use super::url::AppBaseUrl;

/// Standard app CORS config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppCorsConfig {
    allowed_origins: Vec<AppBaseUrl>,
}

impl AppCorsConfig {
    /// Returns allowed CORS origins.
    ///
    /// An empty list means CORS is intentionally disabled for this app.
    pub fn allowed_origins(&self) -> &[AppBaseUrl] {
        self.allowed_origins.as_slice()
    }

    /// Returns whether CORS policy is enabled.
    pub const fn is_enabled(&self) -> bool {
        !self.allowed_origins.is_empty()
    }
}

impl TryFrom<RawCorsConfig> for AppCorsConfig {
    type Error = AppConfigDocumentError;

    fn try_from(raw: RawCorsConfig) -> Result<Self, Self::Error> {
        let mut allowed_origins = Vec::with_capacity(raw.allowed_origins.len());
        for origin in raw.allowed_origins {
            allowed_origins.push(AppBaseUrl::new(origin)?);
        }

        Ok(Self { allowed_origins })
    }
}
