// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validated HTTPS origin and request-target resolution.

use std::fmt;

use url::Url;

use super::{HttpsTransportError, HttpsTransportErrorReason};

const MAXIMUM_ORIGIN_BYTES: usize = 2_048;
pub(super) const MAXIMUM_TARGET_BYTES: usize = 8 * 1_024;

/// An absolute HTTPS origin with no credentials, query, fragment, or path state.
#[derive(Clone, Eq, PartialEq)]
pub struct HttpsOrigin(Url);

impl HttpsOrigin {
    /// Validates and owns an HTTPS origin.
    pub fn new(origin: Url) -> Result<Self, HttpsTransportError> {
        if origin.as_str().len() > MAXIMUM_ORIGIN_BYTES
            || origin.scheme() != "https"
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.path() != "/"
        {
            return Err(HttpsTransportError::local(
                HttpsTransportErrorReason::InvalidOrigin,
            ));
        }
        Ok(Self(origin))
    }

    pub(super) fn resolve(&self, target: &str) -> Result<Url, HttpsTransportError> {
        validate_target(target)?;
        let resolved = self
            .0
            .join(target)
            .map_err(|_| HttpsTransportError::local(HttpsTransportErrorReason::InvalidTarget))?;
        if resolved.scheme() != self.0.scheme()
            || resolved.host_str() != self.0.host_str()
            || resolved.port_or_known_default() != self.0.port_or_known_default()
            || !resolved.username().is_empty()
            || resolved.password().is_some()
            || resolved.query().is_some()
            || resolved.fragment().is_some()
        {
            return Err(HttpsTransportError::local(
                HttpsTransportErrorReason::InvalidTarget,
            ));
        }
        Ok(resolved)
    }
}

impl TryFrom<&str> for HttpsOrigin {
    type Error = HttpsTransportError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let origin = Url::parse(value)
            .map_err(|_| HttpsTransportError::local(HttpsTransportErrorReason::InvalidOrigin))?;
        Self::new(origin)
    }
}

impl fmt::Debug for HttpsOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HttpsOrigin([redacted])")
    }
}

pub(super) fn validate_target(target: &str) -> Result<(), HttpsTransportError> {
    let bytes = target.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAXIMUM_TARGET_BYTES
        || !target.is_ascii()
        || target.starts_with("//")
        || bytes
            .iter()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        || bytes.iter().any(|byte| matches!(byte, b'\\' | b'?' | b'#'))
        || target.contains(';')
        || target.contains(':')
        || has_invalid_percent_escape(bytes)
        || target.split('/').any(is_dot_segment)
    {
        return Err(HttpsTransportError::local(
            HttpsTransportErrorReason::InvalidTarget,
        ));
    }
    Ok(())
}

fn has_invalid_percent_escape(bytes: &[u8]) -> bool {
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let Some(first_index) = index.checked_add(1) else {
                return true;
            };
            let Some(second_index) = index.checked_add(2) else {
                return true;
            };
            let Some(first) = bytes.get(first_index) else {
                return true;
            };
            let Some(second) = bytes.get(second_index) else {
                return true;
            };
            if !first.is_ascii_hexdigit() || !second.is_ascii_hexdigit() {
                return true;
            }
            let decoded = match (
                char::from(*first).to_digit(16),
                char::from(*second).to_digit(16),
            ) {
                (Some(upper), Some(lower)) => upper * 16 + lower,
                _ => return true,
            };
            if decoded < 0x20
                || decoded == 0x7f
                || matches!(decoded, 0x2f | 0x5c | 0x25 | 0x3f | 0x23 | 0x3b)
            {
                return true;
            }
            index = match index.checked_add(3) {
                Some(next) => next,
                None => return true,
            };
        } else {
            index = match index.checked_add(1) {
                Some(next) => next,
                None => return true,
            };
        }
    }
    false
}

fn is_dot_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    let mut decoded_dots = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'.' {
            let Some(next_dots) = decoded_dots.checked_add(1) else {
                return false;
            };
            decoded_dots = next_dots;
            let Some(next_index) = index.checked_add(1) else {
                return false;
            };
            index = next_index;
        } else if bytes[index] == b'%' {
            let Some(first_index) = index.checked_add(1) else {
                return false;
            };
            let Some(second_index) = index.checked_add(2) else {
                return false;
            };
            if bytes.get(first_index) != Some(&b'2')
                || !matches!(bytes.get(second_index), Some(b'e' | b'E'))
            {
                return false;
            }
            let Some(next_dots) = decoded_dots.checked_add(1) else {
                return false;
            };
            decoded_dots = next_dots;
            let Some(next_index) = index.checked_add(3) else {
                return false;
            };
            index = next_index;
        } else {
            return false;
        }
    }
    matches!(decoded_dots, 1 | 2)
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod tests;
