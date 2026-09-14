// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded Valkey environment-name construction and value parsing.

use super::{MAX_ENVIRONMENT_PREFIX_BYTES, config_error};
use crate::{ValkeyConfigErrorReason, ValkeyConfigField, ValkeyResult};

pub(super) fn parse_env_u16(
    name: String,
    default: u16,
    field: ValkeyConfigField,
) -> ValkeyResult<u16> {
    match optional_env(name, field)? {
        Some(value) => value
            .parse::<u16>()
            .map_err(|_error| config_error(field, ValkeyConfigErrorReason::InvalidSyntax)),
        None => Ok(default),
    }
}

pub(super) fn parse_env_u32(
    name: String,
    default: u32,
    field: ValkeyConfigField,
) -> ValkeyResult<u32> {
    match optional_env(name, field)? {
        Some(value) => value
            .parse::<u32>()
            .map_err(|_error| config_error(field, ValkeyConfigErrorReason::InvalidSyntax)),
        None => Ok(default),
    }
}

pub(super) fn parse_env_u64(
    name: String,
    default: u64,
    field: ValkeyConfigField,
) -> ValkeyResult<u64> {
    match optional_env(name, field)? {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_error| config_error(field, ValkeyConfigErrorReason::InvalidSyntax)),
        None => Ok(default),
    }
}

pub(super) fn required_env(name: String, field: ValkeyConfigField) -> ValkeyResult<String> {
    optional_env(name, field)?.ok_or_else(|| config_error(field, ValkeyConfigErrorReason::Empty))
}

pub(super) fn optional_env(name: String, field: ValkeyConfigField) -> ValkeyResult<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(config_error(
            field,
            ValkeyConfigErrorReason::InvalidEncoding,
        )),
    }
}

pub(super) fn env_name(prefix: &str, suffix: &str) -> ValkeyResult<String> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return Err(config_error(
            ValkeyConfigField::EnvironmentPrefix,
            ValkeyConfigErrorReason::Empty,
        ));
    }
    if prefix.len() > MAX_ENVIRONMENT_PREFIX_BYTES {
        return Err(config_error(
            ValkeyConfigField::EnvironmentPrefix,
            ValkeyConfigErrorReason::TooLarge,
        ));
    }
    if !prefix
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(config_error(
            ValkeyConfigField::EnvironmentPrefix,
            ValkeyConfigErrorReason::InvalidSyntax,
        ));
    }

    let capacity = prefix
        .len()
        .checked_add(suffix.len())
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            config_error(
                ValkeyConfigField::EnvironmentPrefix,
                ValkeyConfigErrorReason::TooLarge,
            )
        })?;
    let mut name = String::with_capacity(capacity);
    name.push_str(prefix);
    name.push('_');
    name.push_str(suffix);
    Ok(name)
}
