// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::str::FromStr;

use thiserror::Error;

use super::error::{ConfigError, EnvVarNameErrorReason};

/// Validated environment variable name.
///
/// `reallyme-server-kit` intentionally validates the shape of variable names
/// but does not prescribe app-specific prefixes. Individual apps remain
/// responsible for choosing their own variable names and naming conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnvVarName(&'static str);

impl EnvVarName {
    /// Constructs a validated environment variable name.
    pub fn new(value: &'static str) -> Result<Self, ConfigError> {
        if value.is_empty() {
            return Err(ConfigError::InvalidEnvVarName {
                reason: EnvVarNameErrorReason::Empty,
            });
        }

        if value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Ok(Self(value));
        }

        Err(ConfigError::InvalidEnvVarName {
            reason: EnvVarNameErrorReason::InvalidCharacters,
        })
    }

    /// Returns the validated variable name.
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// Deployment environment for a service instance.
///
/// This belongs in `reallyme-server-kit` because it describes runtime
/// deployment context rather than product semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceEnvironment {
    /// Local developer environment.
    Local,
    /// Shared development environment.
    Dev,
    /// Pre-production staging environment.
    Staging,
    /// Production environment.
    Prod,
}

impl ServiceEnvironment {
    /// Returns whether the environment should be treated as production-like for
    /// configuration fail-closed behavior.
    pub fn is_production_like(self) -> bool {
        matches!(self, Self::Staging | Self::Prod)
    }
}

/// Failure to parse a service deployment environment.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ServiceEnvironmentParseError {
    /// The provided value does not match a supported environment token.
    #[error("service environment value is unsupported")]
    UnsupportedValue,
}

impl FromStr for ServiceEnvironment {
    type Err = ServiceEnvironmentParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // Environment values are parsed case-insensitively so services can
        // accept conventional env-style uppercase values without duplicating
        // normalization logic at each call site.
        if value.eq_ignore_ascii_case("local") {
            Ok(Self::Local)
        } else if value.eq_ignore_ascii_case("dev") {
            Ok(Self::Dev)
        } else if value.eq_ignore_ascii_case("staging") {
            Ok(Self::Staging)
        } else if value.eq_ignore_ascii_case("prod") {
            Ok(Self::Prod)
        } else {
            Err(ServiceEnvironmentParseError::UnsupportedValue)
        }
    }
}

/// Describes whether a configuration value is optional, always required, or
/// required only in production-like environments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigCriticality {
    /// The value is optional in every environment.
    Optional,
    /// The value is required in every environment.
    Required,
    /// The value is optional in lower environments but required in staging and
    /// production. This is the fail-closed setting for production-critical
    /// configuration such as credentials or external service endpoints.
    RequiredInProduction,
}
