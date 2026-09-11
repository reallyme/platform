// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::net::IpAddr;

use super::error::{AppConfigDocumentError, AppConfigDocumentErrorReason};

const MAX_APP_URL_BYTES: usize = 2_048;

/// Validated app base URL or origin.
#[derive(Clone, PartialEq, Eq)]
pub struct AppBaseUrl(String);

impl AppBaseUrl {
    /// Constructs a validated app base URL.
    pub fn new(value: impl Into<String>) -> Result<Self, AppConfigDocumentError> {
        let value = value.into();
        validate_url(value.as_str(), UrlPolicy::HttpsOrLocalHttp)?;

        Ok(Self(value))
    }

    /// Returns the URL string.
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
        let value = value.into();
        validate_url(value.as_str(), UrlPolicy::HttpsOrLocalHttp)?;

        Ok(Self(value))
    }

    /// Returns the URL string.
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

#[derive(Debug, Clone, Copy)]
pub(super) enum UrlPolicy {
    HttpsOrLocalHttp,
    HttpOrHttps,
}

pub(super) fn validate_url(value: &str, policy: UrlPolicy) -> Result<(), AppConfigDocumentError> {
    if value.is_empty() {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::EmptyUrl,
        ));
    }

    if value.len() > MAX_APP_URL_BYTES {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::UrlTooLong,
        ));
    }

    if value.chars().any(char::is_whitespace) {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::ContainsWhitespace,
        ));
    }

    if value.contains('?') {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::ContainsQuery,
        ));
    }

    if value.contains('#') {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::ContainsFragment,
        ));
    }

    let (is_plain_http, without_scheme) = if let Some(rest) = value.strip_prefix("https://") {
        (false, rest)
    } else if let Some(rest) = value.strip_prefix("http://") {
        (true, rest)
    } else {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InvalidUrlScheme,
        ));
    };

    let (authority, path) = without_scheme
        .split_once('/')
        .map_or((without_scheme, ""), |(authority, path)| (authority, path));

    if authority.is_empty() {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::MissingUrlHost,
        ));
    }

    if authority.contains('@') {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::ContainsUserInfo,
        ));
    }

    if !path.is_empty() {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::ContainsPath,
        ));
    }

    parse_url_authority_host(authority)?;

    if matches!(policy, UrlPolicy::HttpsOrLocalHttp)
        && is_plain_http
        && !is_local_authority(authority)
    {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InsecureNonLocalHttpOrigin,
        ));
    }

    Ok(())
}

fn is_local_authority(authority: &str) -> bool {
    let Ok(host) = parse_url_authority_host(authority) else {
        return false;
    };

    if host == "localhost" {
        return true;
    }

    let Ok(host_ip) = host.parse::<IpAddr>() else {
        return false;
    };

    host_ip.is_loopback()
}

fn parse_url_authority_host(authority: &str) -> Result<&str, AppConfigDocumentError> {
    let (host, port_text) = parse_authority_port(authority)?;
    if let Some(port_text) = port_text {
        port_text.parse::<u16>().map_err(|_| {
            AppConfigDocumentError::new(AppConfigDocumentErrorReason::InvalidUrlPort)
        })?;
    }

    if host.is_empty() {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::MissingUrlHost,
        ));
    }

    Ok(host)
}

fn parse_authority_port(authority: &str) -> Result<(&str, Option<&str>), AppConfigDocumentError> {
    if authority.is_empty() {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::MissingUrlHost,
        ));
    }

    if authority.starts_with('[') {
        let Some((host, suffix)) = authority.split_once(']') else {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidUrlPort,
            ));
        };

        if host == "[" || suffix.len() > 1 && !suffix.starts_with(':') {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidUrlPort,
            ));
        }

        if suffix == ":" {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidUrlPort,
            ));
        }

        return Ok((host.trim_start_matches('['), suffix.strip_prefix(':')));
    }

    let Some((host, port_text)) = authority.rsplit_once(':') else {
        return Ok((authority, None));
    };

    if host.is_empty() || host.contains(':') {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InvalidUrlPort,
        ));
    }

    Ok((host, Some(port_text)))
}
