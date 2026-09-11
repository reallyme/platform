// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

use super::super::AppConfigParseErrorReason;

/// Standard app config document error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("standard app JSONC config failed validation")]
pub struct AppConfigDocumentError {
    reason: AppConfigDocumentErrorReason,
}

impl AppConfigDocumentError {
    pub(super) const fn new(reason: AppConfigDocumentErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the typed validation reason.
    pub const fn reason(self) -> AppConfigDocumentErrorReason {
        self.reason
    }
}

/// Standard app config document validation reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppConfigDocumentErrorReason {
    /// JSONC parsing failed.
    Parse {
        /// Typed parse reason.
        reason: AppConfigParseErrorReason,
    },
    /// CORS origins are omitted or empty, meaning CORS is disabled.
    EmptyCorsOrigins,
    /// Numeric config value must be greater than zero.
    MustBeGreaterThanZero,
    /// Numeric config value must not exceed the configured maximum.
    MustNotExceedMaximum,
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
    /// App-specific custom config failed validation after standard-envelope parsing.
    InvalidCustomConfig,
    /// App-level reflection is not supported by the standard config envelope.
    ReflectionNotSupported,
}
