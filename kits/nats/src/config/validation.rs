// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! NATS endpoint, subject, and transport-policy validation.

use std::net::IpAddr;
use std::str;

use url::Url;

use super::{JetStreamTlsPolicy, MAX_NATS_URL_BYTES};
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
        JetStreamTlsPolicy::Required | JetStreamTlsPolicy::Optional
    ) && !url.scheme().eq_ignore_ascii_case("tls")
    {
        return Err(JetStreamError::InvalidConfiguration);
    }

    let scheme = url.scheme().to_ascii_lowercase();
    if !matches!(scheme.as_str(), "nats" | "tls") {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if url.host().is_none() {
        return Err(JetStreamError::InvalidConfiguration);
    }
    if tls_policy == JetStreamTlsPolicy::Disabled
        && scheme == "nats"
        && !url.host_str().is_some_and(is_loopback_host)
    {
        // A caller selecting the legacy plaintext policy must not turn a
        // public endpoint into a cleartext credential transport.
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
    let subject = validate_subject_text(value, max_bytes, required)?;
    if !subject.is_empty()
        && !subject.split('.').all(|token| {
            !token.is_empty()
                && token
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
    {
        return Err(JetStreamError::InvalidConfiguration);
    }

    Ok(subject)
}

pub(super) fn validate_filter_subject(
    value: &str,
    max_bytes: usize,
    required: bool,
) -> Result<String, JetStreamError> {
    let subject = validate_subject_text(value, max_bytes, required)?;
    if !subject.is_empty() {
        let mut tokens = subject.split('.').peekable();
        while let Some(token) = tokens.next() {
            if token.is_empty()
                || (token == ">" && tokens.peek().is_some())
                || (token != "*"
                    && token != ">"
                    && !token
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
            {
                return Err(JetStreamError::InvalidConfiguration);
            }
        }
    }
    Ok(subject)
}

pub(super) fn validate_component(
    value: &str,
    max_bytes: usize,
    required: bool,
) -> Result<String, JetStreamError> {
    if value.is_empty() {
        return if required {
            Err(JetStreamError::InvalidConfiguration)
        } else {
            Ok(String::new())
        };
    }

    if value.len() > max_bytes
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(JetStreamError::InvalidConfiguration);
    }

    Ok(value.to_owned())
}

fn validate_subject_text(
    value: &str,
    max_bytes: usize,
    required: bool,
) -> Result<String, JetStreamError> {
    if value.is_empty() {
        return if required {
            Err(JetStreamError::InvalidConfiguration)
        } else {
            Ok(String::new())
        };
    }
    if value.len() > max_bytes {
        return Err(JetStreamError::InvalidConfiguration);
    }
    Ok(value.to_owned())
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
        if is_loopback_host(&host) {
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

fn is_loopback_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    // The URL crate reports `nats:` IPv4 literals as domains and encloses
    // IPv6 literals in brackets. Parse both explicitly before deciding TLS.
    let unbracketed = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = unbracketed.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(ipv4) => ipv4.is_loopback(),
            IpAddr::V6(ipv6) => {
                ipv6.is_loopback()
                    || ipv6
                        .to_ipv4_mapped()
                        .is_some_and(|mapped| mapped.is_loopback())
            }
        };
    }
    false
}
