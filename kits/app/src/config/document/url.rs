// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use url::{Host, ParseError, Url};

use super::error::{AppConfigDocumentError, AppConfigDocumentErrorReason};
use super::service_endpoint::AppServiceEndpointUrl;

const MAX_APP_URL_BYTES: usize = 2_048;

/// Validated app base URL or origin.
#[derive(Clone, PartialEq, Eq)]
pub struct AppBaseUrl(String);

impl AppBaseUrl {
    /// Constructs a validated app base URL.
    pub fn new(value: impl Into<String>) -> Result<Self, AppConfigDocumentError> {
        validate_secure_url(&value.into()).map(Self)
    }

    /// Returns the canonical URL string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppBaseUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AppBaseUrl")
            .field(&"[REDACTED]")
            .finish()
    }
}

/// Validated downstream base URL.
#[derive(Clone, PartialEq, Eq)]
pub struct AppDownstreamBaseUrl(String);

impl AppDownstreamBaseUrl {
    /// Constructs a validated downstream base URL.
    pub fn new(value: impl Into<String>) -> Result<Self, AppConfigDocumentError> {
        validate_secure_url(&value.into()).map(Self)
    }

    pub(super) fn from_endpoint(endpoint: &AppServiceEndpointUrl) -> Self {
        // Both wrappers enforce the same URL policy. Retain the canonical
        // endpoint without a second parser that could drift or discard it.
        Self(endpoint.as_str().to_owned())
    }

    /// Returns the canonical URL string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppDownstreamBaseUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AppDownstreamBaseUrl")
            .field(&"[REDACTED]")
            .finish()
    }
}

pub(super) fn validate_secure_url(value: &str) -> Result<String, AppConfigDocumentError> {
    let invalid = AppConfigDocumentError::new;
    if value.is_empty() {
        return Err(invalid(AppConfigDocumentErrorReason::EmptyUrl));
    }
    if value.len() > MAX_APP_URL_BYTES {
        return Err(invalid(AppConfigDocumentErrorReason::UrlTooLong));
    }
    if value.chars().any(char::is_whitespace) {
        return Err(invalid(AppConfigDocumentErrorReason::ContainsWhitespace));
    }
    // WHATWG URLs turn backslashes into slashes for special schemes. Reject
    // the ambiguous spelling before parsing so policy checks see the same
    // authority as the HTTP client.
    if value.contains('\\') {
        return Err(invalid(AppConfigDocumentErrorReason::InvalidUrlScheme));
    }
    if value.contains('?') {
        return Err(invalid(AppConfigDocumentErrorReason::ContainsQuery));
    }
    if value.contains('#') {
        return Err(invalid(AppConfigDocumentErrorReason::ContainsFragment));
    }
    let authority = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .ok_or_else(|| invalid(AppConfigDocumentErrorReason::InvalidUrlScheme))?
        .split('/')
        .next()
        .ok_or_else(|| invalid(AppConfigDocumentErrorReason::MissingUrlHost))?;
    if authority.is_empty() {
        return Err(invalid(AppConfigDocumentErrorReason::MissingUrlHost));
    }
    if authority.contains('@') {
        return Err(invalid(AppConfigDocumentErrorReason::ContainsUserInfo));
    }

    let parsed = Url::parse(value).map_err(|error| match error {
        ParseError::InvalidPort | ParseError::InvalidIpv6Address => {
            invalid(AppConfigDocumentErrorReason::InvalidUrlPort)
        }
        _ => invalid(AppConfigDocumentErrorReason::MissingUrlHost),
    })?;
    let host = parsed
        .host()
        .ok_or_else(|| invalid(AppConfigDocumentErrorReason::MissingUrlHost))?;
    if parsed.path() != "/" {
        return Err(invalid(AppConfigDocumentErrorReason::ContainsPath));
    }
    if parsed.query().is_some() {
        return Err(invalid(AppConfigDocumentErrorReason::ContainsQuery));
    }
    if parsed.fragment().is_some() {
        return Err(invalid(AppConfigDocumentErrorReason::ContainsFragment));
    }
    if parsed.port() == Some(0) {
        return Err(invalid(AppConfigDocumentErrorReason::InvalidUrlPort));
    }
    if parsed.scheme() == "http" && !is_loopback_host(host) {
        return Err(invalid(
            AppConfigDocumentErrorReason::InsecureNonLocalHttpOrigin,
        ));
    }

    // Store the parser's canonical origin so equivalent configured endpoints
    // deduplicate and browser Origin values match exactly.
    Ok(parsed.origin().ascii_serialization())
}

fn is_loopback_host(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => {
            address.is_loopback() || address.to_ipv4_mapped().is_some_and(|ip| ip.is_loopback())
        }
    }
}
