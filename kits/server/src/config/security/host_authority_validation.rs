// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Borrowed HTTP host-authority validation and parsing.

use axum::http::HeaderValue;

use super::{HostAuthorityParts, MAX_HOST_AUTHORITY_BYTES, invalid_host_authority};
use crate::config::{ConfigError, ConfigValidationErrorReason, NetworkPort};

pub(super) fn validate_host_authority_syntax(value: &str) -> Result<(), ConfigError> {
    if value.is_empty() {
        return Err(invalid_host_authority(
            ConfigValidationErrorReason::MustBeNonEmpty,
        ));
    }

    if value.len() > MAX_HOST_AUTHORITY_BYTES {
        return Err(invalid_host_authority(
            ConfigValidationErrorReason::MustBeLessThanOrEqualToMaximum,
        ));
    }

    if value.chars().any(char::is_whitespace)
        || value.contains('/')
        || value.contains('?')
        || value.contains('#')
        || value.contains('@')
        || value.contains("://")
    {
        return Err(invalid_host_authority(
            ConfigValidationErrorReason::InvalidOrigin,
        ));
    }

    HeaderValue::from_str(value)
        .map_err(|_| invalid_host_authority(ConfigValidationErrorReason::InvalidHeaderValue))?;

    Ok(())
}

pub(super) fn parse_host_authority_parts(
    authority: &str,
) -> Result<HostAuthorityParts<'_>, ConfigError> {
    if let Some(remainder) = authority.strip_prefix('[') {
        let Some((host, suffix)) = remainder.split_once(']') else {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::InvalidHeaderValue,
            ));
        };
        if host.is_empty() {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::InvalidHeaderValue,
            ));
        }

        let port = if suffix.is_empty() {
            None
        } else {
            let Some(port_value) = suffix.strip_prefix(':') else {
                return Err(invalid_host_authority(
                    ConfigValidationErrorReason::InvalidHeaderValue,
                ));
            };
            Some(parse_host_authority_port(port_value)?)
        };

        return Ok(HostAuthorityParts { host, port });
    }

    if let Some((host, port_value)) = authority.rsplit_once(':')
        && !host.contains(':')
        && !port_value.is_empty()
        && port_value.chars().all(|char| char.is_ascii_digit())
    {
        return Ok(HostAuthorityParts {
            host,
            port: Some(parse_host_authority_port(port_value)?),
        });
    }

    if authority.contains(':') {
        return Err(invalid_host_authority(
            ConfigValidationErrorReason::InvalidHeaderValue,
        ));
    }

    Ok(HostAuthorityParts {
        host: authority,
        port: None,
    })
}

fn parse_host_authority_port(value: &str) -> Result<NetworkPort, ConfigError> {
    let port = value
        .parse::<u16>()
        .map_err(|_| invalid_host_authority(ConfigValidationErrorReason::InvalidHeaderValue))?;

    NetworkPort::new(port)
}
