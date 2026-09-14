// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use super::error::{AppConfigDocumentError, AppConfigDocumentErrorReason};
use super::raw::RawCookieConfig;

const MAX_COOKIE_DOMAIN_BYTES: usize = 255;

/// Standard app cookie/session transport config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppCookieConfig {
    secure: bool,
    domain: Option<AppCookieDomain>,
    same_site_policy: AppCookieSameSitePolicy,
}

impl AppCookieConfig {
    /// Returns whether cookies must be emitted with the `Secure` attribute.
    pub const fn secure(&self) -> bool {
        self.secure
    }

    /// Returns the optional cookie domain override.
    pub const fn domain(&self) -> Option<&AppCookieDomain> {
        self.domain.as_ref()
    }

    /// Returns the validated same-site policy.
    pub const fn same_site_policy(&self) -> AppCookieSameSitePolicy {
        self.same_site_policy
    }
}

impl TryFrom<RawCookieConfig> for AppCookieConfig {
    type Error = AppConfigDocumentError;

    fn try_from(raw: RawCookieConfig) -> Result<Self, Self::Error> {
        let secure = raw.secure.unwrap_or(true);
        let same_site_policy = raw
            .same_site_policy
            .as_deref()
            .map(AppCookieSameSitePolicy::parse)
            .transpose()?
            .unwrap_or(AppCookieSameSitePolicy::Lax);

        if same_site_policy == AppCookieSameSitePolicy::None && !secure {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::CookieSameSiteNoneRequiresSecure,
            ));
        }

        Ok(Self {
            secure,
            domain: raw.domain.map(AppCookieDomain::new).transpose()?,
            same_site_policy,
        })
    }
}

/// Validated cookie domain attribute value.
#[derive(Clone, PartialEq, Eq)]
pub struct AppCookieDomain(String);

impl AppCookieDomain {
    /// Constructs a validated cookie domain.
    pub fn new(value: impl Into<String>) -> Result<Self, AppConfigDocumentError> {
        let value = value.into();
        validate_cookie_domain(value.as_str())?;
        Ok(Self(value.to_ascii_lowercase()))
    }

    /// Returns the normalized cookie domain.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppCookieDomain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AppCookieDomain")
            .field(&"[REDACTED]")
            .finish()
    }
}

/// Validated same-site cookie transport policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCookieSameSitePolicy {
    /// Restrict cookies to same-site requests only.
    Strict,
    /// Allow top-level cross-site navigations while blocking ambient use.
    Lax,
    /// Allow cross-site cookie transport. Requires `Secure`.
    None,
}

impl AppCookieSameSitePolicy {
    fn parse(value: &str) -> Result<Self, AppConfigDocumentError> {
        match value {
            "strict" => Ok(Self::Strict),
            "lax" => Ok(Self::Lax),
            "none" => Ok(Self::None),
            _ => Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidCookieSameSitePolicy,
            )),
        }
    }
}

fn validate_cookie_domain(value: &str) -> Result<(), AppConfigDocumentError> {
    if value.is_empty() {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InvalidCookieDomain,
        ));
    }

    if value.len() > MAX_COOKIE_DOMAIN_BYTES {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InvalidCookieDomain,
        ));
    }

    if value.chars().any(char::is_whitespace)
        || value.contains('/')
        || value.contains(':')
        || value.contains('@')
        || value.starts_with('.')
        || value.ends_with('.')
    {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InvalidCookieDomain,
        ));
    }

    let mut saw_dot = false;
    for label in value.split('.') {
        if label.is_empty() {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidCookieDomain,
            ));
        }

        saw_dot = true;
        if label.starts_with('-') || label.ends_with('-') {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidCookieDomain,
            ));
        }

        if !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidCookieDomain,
            ));
        }
    }

    if !saw_dot && !value.eq_ignore_ascii_case("localhost") {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InvalidCookieDomain,
        ));
    }

    Ok(())
}
