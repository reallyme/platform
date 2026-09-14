// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::cookies::AppCookieConfig;
use super::cors::AppCorsConfig;
use super::downstream::AppDownstreamConfig;
use super::error::{AppConfigDocumentError, AppConfigDocumentErrorReason};
use super::raw::RawAppJsoncConfigDocument;
use super::url::AppBaseUrl;
use crate::config::parse_jsonc_config;

/// Standard host-neutral app JSONC config document.
///
/// App-kit owns the common envelope so apps do not invent incompatible config
/// shapes. App crates may add custom top-level fields through `TCustom`, but
/// they still receive the standardized app/runtime-adjacent config through
/// validated immutable types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppJsoncConfigDocument<TCustom> {
    public_base_url: Option<AppBaseUrl>,
    cors: AppCorsConfig,
    cookies: Option<AppCookieConfig>,
    reflection_enabled: bool,
    downstream: AppDownstreamConfig,
    custom: TCustom,
}

impl<TCustom> AppJsoncConfigDocument<TCustom> {
    /// Parses and validates a standard app JSONC config document.
    pub fn from_jsonc_str(value: &str) -> Result<Self, AppConfigDocumentError>
    where
        TCustom: DeserializeOwned,
    {
        let raw =
            parse_jsonc_config::<RawAppJsoncConfigDocument<TCustom>>(value).map_err(|error| {
                AppConfigDocumentError::new(AppConfigDocumentErrorReason::Parse {
                    reason: error.reason(),
                })
            })?;

        Self::try_from(raw)
    }

    /// Returns the optional public base URL.
    pub const fn public_base_url(&self) -> Option<&AppBaseUrl> {
        self.public_base_url.as_ref()
    }

    /// Returns CORS config.
    pub const fn cors(&self) -> &AppCorsConfig {
        &self.cors
    }

    /// Returns optional cookie/session transport config.
    pub const fn cookies(&self) -> Option<&AppCookieConfig> {
        self.cookies.as_ref()
    }

    /// Returns whether reflection is requested.
    pub const fn reflection_enabled(&self) -> bool {
        self.reflection_enabled
    }

    /// Returns downstream endpoint config.
    pub const fn downstream(&self) -> &AppDownstreamConfig {
        &self.downstream
    }

    /// Returns app-specific custom config.
    pub const fn custom(&self) -> &TCustom {
        &self.custom
    }
}

impl<TCustom> TryFrom<RawAppJsoncConfigDocument<TCustom>> for AppJsoncConfigDocument<TCustom> {
    type Error = AppConfigDocumentError;

    fn try_from(raw: RawAppJsoncConfigDocument<TCustom>) -> Result<Self, Self::Error> {
        if raw.reflection_enabled {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::ReflectionNotSupported,
            ));
        }

        Ok(Self {
            public_base_url: raw.public_base_url.map(AppBaseUrl::new).transpose()?,
            cors: AppCorsConfig::try_from(raw.cors)?,
            cookies: raw.cookies.map(AppCookieConfig::try_from).transpose()?,
            reflection_enabled: raw.reflection_enabled,
            downstream: AppDownstreamConfig::try_from(raw.downstream)?,
            custom: raw.custom,
        })
    }
}

/// Parses and validates a standard app JSONC config document.
pub fn parse_app_jsonc_config_document<TCustom>(
    value: &str,
) -> Result<AppJsoncConfigDocument<TCustom>, AppConfigDocumentError>
where
    TCustom: DeserializeOwned,
{
    AppJsoncConfigDocument::from_jsonc_str(value)
}

/// Empty custom config for apps that only use the standard envelope.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoAppCustomConfig {}
