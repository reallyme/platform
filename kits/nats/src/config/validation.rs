// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! NATS endpoint, subject, and transport-policy validation.

use std::net::IpAddr;
use std::str::{self, FromStr};

use url::Url;

use super::{JetStreamTlsPolicy, LOCAL_SUBJECT_SUFFIX, MAX_NATS_URL_BYTES};
use crate::error::JetStreamError;

pub(crate) fn validate_nats_url(
    value: &str,
    required: bool,
    tls_policy: JetStreamTlsPolicy,
) -> Result<String, JetStreamError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return if required {
            Err(JetStreamError::InvalidConfiguration)
        } else {
            Ok(String::new())
        };
    }

    if trimmed.len() > MAX_NATS_URL_BYTES || trimmed.chars().any(char::is_whitespace) {
        return Err(JetStreamError::InvalidConfiguration);
    }

    let url = Url::parse(trimmed).map_err(|_| JetStreamError::InvalidConfiguration)?;

    if url.username() != "" || url.password().is_some() {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if url.scheme().is_empty() {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if matches!(
        tls_policy,
        JetStreamTlsPolicy::Required if !url.scheme().eq_ignore_ascii_case("tls")
    ) {
        return Err(JetStreamError::InvalidConfiguration);
    }

    let scheme = url.scheme().to_ascii_lowercase();
    if !matches!(scheme.as_str(), "nats" | "tls") {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if url.host().is_none() {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if !url.path().is_empty() && url.path() != "/" {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if url.query().is_some() || url.fragment().is_some() {
        return Err(JetStreamError::InvalidConfiguration);
    }

    Ok(trimmed.to_owned())
}

pub(super) fn validate_publish_subject(
    value: &str,
    max_bytes: usize,
    required: bool,
) -> Result<String, JetStreamError> {
    let subject = validate_component(value, max_bytes, required)?;
    if subject.contains('*') || subject.contains('>') {
        return Err(JetStreamError::InvalidConfiguration);
    }

    Ok(subject)
}

pub(super) fn validate_filter_subject(
    value: &str,
    max_bytes: usize,
    required: bool,
) -> Result<String, JetStreamError> {
    validate_component(value, max_bytes, required)
}

pub(super) fn validate_component(
    value: &str,
    max_bytes: usize,
    required: bool,
) -> Result<String, JetStreamError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return if required {
            Err(JetStreamError::InvalidConfiguration)
        } else {
            Ok(String::new())
        };
    }

    if trimmed.len() > max_bytes || trimmed.chars().any(char::is_whitespace) {
        return Err(JetStreamError::InvalidConfiguration);
    }

    Ok(trimmed.to_owned())
}

impl JetStreamTlsPolicy {
    /// Derives the default TLS policy for a URL from host locality and scheme.
    pub fn derive_from_url(nats_url: &str) -> Result<Self, JetStreamError> {
        if nats_url.is_empty() {
            return Ok(Self::Optional);
        }

        let trimmed = nats_url.trim();
        let host = parse_nats_host(trimmed).ok_or(JetStreamError::InvalidConfiguration)?;
        let scheme = parse_nats_scheme(trimmed).ok_or(JetStreamError::InvalidConfiguration)?;
        if scheme.eq_ignore_ascii_case("tls") {
            return Ok(Self::Required);
        }
        if is_private_network_host(&host) {
            Ok(Self::Disabled)
        } else {
            Ok(Self::Required)
        }
    }
}

fn parse_nats_scheme(value: &str) -> Option<String> {
    Url::parse(value).ok().and_then(|parsed| {
        if parsed.scheme().is_empty() {
            return None;
        }

        Some(parsed.scheme().to_owned())
    })
}

fn parse_nats_host(value: &str) -> Option<String> {
    Url::parse(value)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
}

pub(crate) fn redact_nats_url(value: &str) -> String {
    let Ok(url) = Url::parse(value) else {
        return String::from("<invalid-url>");
    };

    let mut redacted = String::new();
    redacted.push_str(url.scheme());
    redacted.push_str("://");

    if let Some(host) = url.host_str() {
        redacted.push_str(host);
    }

    if let Some(port) = url.port() {
        redacted.push(':');
        redacted.push_str(&port.to_string());
    }

    if url.host().is_none() {
        return redacted;
    }

    if url.path() != "/" && !url.path().is_empty() {
        redacted.push_str(url.path());
    }

    redacted
}

fn is_private_network_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let host = host.to_ascii_lowercase();
    if host.ends_with(LOCAL_SUBJECT_SUFFIX) {
        return true;
    }

    if let Ok(ip) = IpAddr::from_str(&host) {
        return match ip {
            IpAddr::V4(ipv4) => ipv4.is_loopback() || ipv4.is_private() || ipv4.is_unspecified(),
            IpAddr::V6(ipv6) => ipv6.is_loopback() || ipv6.is_unspecified(),
        };
    }

    !host.contains('.')
}
