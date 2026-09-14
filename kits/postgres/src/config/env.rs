// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded PostgreSQL environment-name construction and value parsing.

use super::{MAX_ENVIRONMENT_PREFIX_BYTES, config_error};
use crate::error::{PostgresConfigErrorReason, PostgresConfigField, PostgresResult};

pub(super) fn parse_env_u32(
    name: String,
    default: u32,
    field: PostgresConfigField,
) -> PostgresResult<u32> {
    match optional_env(name, field)? {
        Some(value) => value
            .parse::<u32>()
            .map_err(|_error| config_error(field, PostgresConfigErrorReason::InvalidSyntax)),
        None => Ok(default),
    }
}

pub(super) fn parse_env_u64(
    name: String,
    default: u64,
    field: PostgresConfigField,
) -> PostgresResult<u64> {
    match optional_env(name, field)? {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_error| config_error(field, PostgresConfigErrorReason::InvalidSyntax)),
        None => Ok(default),
    }
}

pub(super) fn optional_env(
    name: String,
    field: PostgresConfigField,
) -> PostgresResult<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(config_error(
            field,
            PostgresConfigErrorReason::InvalidEncoding,
        )),
    }
}

pub(super) fn env_name(prefix: &str, suffix: &str) -> PostgresResult<String> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return Err(config_error(
            PostgresConfigField::EnvironmentPrefix,
            PostgresConfigErrorReason::Empty,
        ));
    }
    if prefix.len() > MAX_ENVIRONMENT_PREFIX_BYTES {
        return Err(config_error(
            PostgresConfigField::EnvironmentPrefix,
            PostgresConfigErrorReason::TooLarge,
        ));
    }
    if !prefix
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(config_error(
            PostgresConfigField::EnvironmentPrefix,
            PostgresConfigErrorReason::InvalidSyntax,
        ));
    }

    let capacity = prefix
        .len()
        .checked_add(suffix.len())
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            config_error(
                PostgresConfigField::EnvironmentPrefix,
                PostgresConfigErrorReason::TooLarge,
            )
        })?;
    let mut name = String::with_capacity(capacity);
    name.push_str(prefix);
    name.push('_');
    name.push_str(suffix);
    Ok(name)
}
