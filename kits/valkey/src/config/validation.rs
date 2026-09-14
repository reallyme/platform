// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Valkey endpoint, key-prefix, credential, and TLS validation.

use std::net::IpAddr;
use std::str::FromStr;

use secrecy::{ExposeSecret, SecretString};

use super::{
    MAX_CREDENTIAL_BYTES, MAX_HOST_BYTES, MAX_KEY_PREFIX_BYTES, MAX_TLS_CA_PATH_BYTES,
    ValkeyTlsTrust, config_error,
};
use crate::{ValkeyConfigErrorReason, ValkeyConfigField, ValkeyResult};

pub(super) fn validate_host(value: &str) -> ValkeyResult<()> {
    if value.is_empty() {
        return Err(config_error(
            ValkeyConfigField::Host,
            ValkeyConfigErrorReason::Empty,
        ));
    }
    if value.len() > MAX_HOST_BYTES {
        return Err(config_error(
            ValkeyConfigField::Host,
            ValkeyConfigErrorReason::TooLarge,
        ));
    }
    let valid_dns = value.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    });
    if IpAddr::from_str(value).is_err() && !valid_dns {
        return Err(config_error(
            ValkeyConfigField::Host,
            ValkeyConfigErrorReason::InvalidSyntax,
        ));
    }
    Ok(())
}

pub(super) fn validate_key_prefix(value: &str) -> ValkeyResult<()> {
    if value.is_empty() {
        return Err(config_error(
            ValkeyConfigField::KeyPrefix,
            ValkeyConfigErrorReason::Empty,
        ));
    }
    if value.len() > MAX_KEY_PREFIX_BYTES {
        return Err(config_error(
            ValkeyConfigField::KeyPrefix,
            ValkeyConfigErrorReason::TooLarge,
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':'))
    {
        return Err(config_error(
            ValkeyConfigField::KeyPrefix,
            ValkeyConfigErrorReason::InvalidSyntax,
        ));
    }
    Ok(())
}

pub(super) fn validate_tls_trust(value: &ValkeyTlsTrust) -> ValkeyResult<()> {
    let Some(path) = value.custom_root_certificate() else {
        return Ok(());
    };
    if path.as_os_str().is_empty() {
        return Err(config_error(
            ValkeyConfigField::TlsCaCertificatePath,
            ValkeyConfigErrorReason::Empty,
        ));
    }
    if path.as_os_str().as_encoded_bytes().len() > MAX_TLS_CA_PATH_BYTES {
        return Err(config_error(
            ValkeyConfigField::TlsCaCertificatePath,
            ValkeyConfigErrorReason::TooLarge,
        ));
    }
    Ok(())
}

pub(super) fn validate_optional_secret(
    value: Option<&SecretString>,
    field: ValkeyConfigField,
) -> ValkeyResult<()> {
    if let Some(secret) = value {
        if secret.expose_secret().is_empty() {
            return Err(config_error(field, ValkeyConfigErrorReason::Empty));
        }
        if secret.expose_secret().len() > MAX_CREDENTIAL_BYTES {
            return Err(config_error(field, ValkeyConfigErrorReason::TooLarge));
        }
    }
    Ok(())
}
