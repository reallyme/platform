// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

use super::super::AppConfigParseErrorReason;

/// Standard app config document error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("standard app JSONC config field {field:?} failed validation: {reason:?}")]
pub struct AppConfigDocumentError {
    field: AppConfigDocumentField,
    reason: AppConfigDocumentErrorReason,
}

impl AppConfigDocumentError {
    pub(super) const fn new(reason: AppConfigDocumentErrorReason) -> Self {
        Self {
            field: AppConfigDocumentField::Document,
            reason,
        }
    }

    pub(super) const fn with_field(mut self, field: AppConfigDocumentField) -> Self {
        self.field = field;
        self
    }

    /// Returns the reviewed config field associated with the failure.
    pub const fn field(self) -> AppConfigDocumentField {
        self.field
    }

    /// Returns the typed validation reason.
    pub const fn reason(self) -> AppConfigDocumentErrorReason {
        self.reason
    }
}

/// Fixed field paths used in config diagnostics without echoing input values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppConfigDocumentField {
    /// The document could not be parsed or its field is unavailable.
    Document,
    /// `public_base_url`.
    PublicBaseUrl,
    /// `cors.allowed_origins`.
    CorsAllowedOrigins,
    /// `cookies.secure`.
    CookiesSecure,
    /// `cookies.domain`.
    CookiesDomain,
    /// `cookies.same_site_policy`.
    CookiesSameSitePolicy,
    /// `reflection_enabled`.
    ReflectionEnabled,
    /// `downstream`.
    Downstream,
}

/// Standard app config document validation reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppConfigDocumentErrorReason {
    /// JSONC parsing failed.
    Parse {
        /// Typed parse reason.
        reason: AppConfigParseErrorReason,
    },
    /// URL scheme was unsupported.
    InvalidUrlScheme,
    /// URL value was empty.
    EmptyUrl,
    /// URL value exceeded the platform maximum length.
    UrlTooLong,
    /// URL did not contain a host.
    MissingUrlHost,
    /// URL contained whitespace.
    ContainsWhitespace,
    /// URL contained a query string.
    ContainsQuery,
    /// URL contained a fragment.
    ContainsFragment,
    /// URL port was invalid or out-of-range.
    InvalidUrlPort,
    /// URL contained credentials/userinfo.
    ContainsUserInfo,
    /// URL contained a non-root path.
    ContainsPath,
    /// Plain HTTP was used for a public/non-local origin.
    InsecureNonLocalHttpOrigin,
    /// Cookie domain failed validation.
    InvalidCookieDomain,
    /// Same-site cookie policy token was unsupported.
    InvalidCookieSameSitePolicy,
    /// `SameSite=None` requires `Secure`.
    CookieSameSiteNoneRequiresSecure,
    /// Downstream endpoint name failed validation.
    InvalidDownstreamName,
    /// Downstream endpoint source shape was invalid.
    InvalidServiceEndpointSource,
    /// Endpoint selection token was unsupported.
    InvalidEndpointSelection,
    /// Service-locator mode was unsupported.
    InvalidLocatorMode,
    /// Service-locator identifier token was invalid.
    InvalidLocatorIdentifier,
    /// Service-locator tag set was invalid.
    InvalidLocatorTag,
    /// App-level reflection is not supported by the standard config envelope.
    ReflectionNotSupported,
}
