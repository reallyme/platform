// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::str::FromStr;

use axum::http::{HeaderValue, Uri};

use super::environment::ServiceEnvironment;
use super::error::{ConfigError, ConfigValidationErrorReason, CorsConfigField};

/// Shared CORS policy configuration.
///
/// This type only models generic transport policy. Service-specific route or
/// product authorization policy must not be encoded here.
#[derive(Clone, PartialEq, Eq)]
pub enum CorsConfig {
    /// Disables cross-origin access by default.
    NoCors,
    /// Allows requests from one or more exact origins.
    ExactOrigins(ExactCorsOrigins),
    /// Allows any origin, but should only be used intentionally in local or
    /// shared development environments.
    AnyForDevelopmentOnly,
}

impl CorsConfig {
    /// Disables CORS response headers.
    pub fn no_cors() -> Self {
        Self::NoCors
    }

    /// Allows exactly one origin after validating origin syntax and header
    /// compatibility.
    pub fn allow_exact_origin(origin: &str) -> Result<Self, ConfigError> {
        Ok(Self::ExactOrigins(ExactCorsOrigins::new(vec![
            ExactCorsOrigin::new(origin)?,
        ])?))
    }

    /// Allows one or more exact origins after validating syntax and header
    /// compatibility for each configured origin.
    pub fn allow_exact_origins(origins: Vec<String>) -> Result<Self, ConfigError> {
        let validated = origins
            .into_iter()
            .map(|origin| ExactCorsOrigin::new(origin.as_str()))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self::ExactOrigins(ExactCorsOrigins::new(validated)?))
    }

    /// Allows any origin for explicitly non-production environments.
    pub fn allow_any_for_development_only(
        service_environment: ServiceEnvironment,
    ) -> Result<Self, ConfigError> {
        if service_environment.is_production_like() {
            return Err(ConfigError::CorsPolicyDisallowedInEnvironment {
                service_environment,
            });
        }

        Ok(Self::AnyForDevelopmentOnly)
    }
}

impl fmt::Debug for CorsConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCors => formatter.write_str("CorsConfig::NoCors"),
            Self::ExactOrigins(origins) => formatter
                .debug_tuple("CorsConfig::ExactOrigins")
                .field(origins)
                .finish(),
            Self::AnyForDevelopmentOnly => formatter.write_str("CorsConfig::AnyForDevelopmentOnly"),
        }
    }
}

/// Validated collection of exact CORS origins.
#[derive(Clone, PartialEq, Eq)]
pub struct ExactCorsOrigins {
    origins: Vec<ExactCorsOrigin>,
}

impl ExactCorsOrigins {
    /// Constructs a validated exact-origin collection.
    pub fn new(origins: Vec<ExactCorsOrigin>) -> Result<Self, ConfigError> {
        if origins.is_empty() {
            return Err(ConfigError::InvalidCorsConfig {
                field: CorsConfigField::AllowOrigin,
                reason: ConfigValidationErrorReason::MustBeNonEmpty,
            });
        }

        Ok(Self { origins })
    }

    /// Returns the configured exact origins.
    pub fn as_slice(&self) -> &[ExactCorsOrigin] {
        self.origins.as_slice()
    }
}

impl fmt::Debug for ExactCorsOrigins {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactCorsOrigins")
            .field("count", &self.origins.len())
            .finish()
    }
}

/// Validated exact CORS origin.
///
/// The value is not a secret, but `Debug` remains conservative so startup and
/// config-derived logs do not blindly echo arbitrary configuration values. If
/// we later need richer operational summaries, they should expose a sanitized
/// and intentionally documented representation rather than relying on generic
/// `Debug` output.
#[derive(Clone, PartialEq, Eq)]
pub struct ExactCorsOrigin {
    raw: String,
    header_value: HeaderValue,
}

impl ExactCorsOrigin {
    /// Constructs a validated exact CORS origin.
    ///
    /// CORS origins are serialized as scheme plus authority, for example
    /// `https://app.reallyme.net` or `http://localhost:3000`. Only `http` and
    /// `https` origins are accepted. Query strings, fragments, credentials,
    /// whitespace, arbitrary header-safe strings, and paths other than empty or
    /// `/` are rejected.
    ///
    /// Localhost and `127.0.0.1` origins are valid syntax for local/dev
    /// services, but CORS remains explicit: the default policy is still
    /// `NoCors`, and each service must intentionally opt into any origin.
    pub fn new(origin: &str) -> Result<Self, ConfigError> {
        if origin.is_empty() {
            return Err(ConfigError::InvalidCorsConfig {
                field: CorsConfigField::AllowOrigin,
                reason: ConfigValidationErrorReason::MustBeNonEmpty,
            });
        }

        validate_origin_syntax(origin)?;

        let header_value =
            HeaderValue::from_str(origin).map_err(|_| ConfigError::InvalidCorsConfig {
                field: CorsConfigField::AllowOrigin,
                reason: ConfigValidationErrorReason::InvalidHeaderValue,
            })?;

        Ok(Self {
            raw: origin.to_owned(),
            header_value,
        })
    }

    /// Returns the exact origin as a header value.
    pub fn as_header_value(&self) -> &HeaderValue {
        &self.header_value
    }

    /// Returns the validated exact origin as a string.
    ///
    /// Origins are not secrets, but callers should still avoid logging
    /// arbitrary config values unless they are intentionally part of a startup
    /// summary or operator-facing diagnostic surface.
    pub fn as_str(&self) -> &str {
        self.raw.as_str()
    }
}

impl fmt::Debug for ExactCorsOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ExactCorsOrigin(<redacted-origin-policy-value-not-secret>)")
    }
}

fn validate_origin_syntax(origin: &str) -> Result<(), ConfigError> {
    if origin.chars().any(char::is_whitespace) || origin.contains('?') || origin.contains('#') {
        return Err(invalid_origin_error());
    }

    let uri = Uri::from_str(origin).map_err(|_| invalid_origin_error())?;
    let Some(scheme) = uri.scheme_str() else {
        return Err(invalid_origin_error());
    };
    let Some(authority) = uri.authority() else {
        return Err(invalid_origin_error());
    };

    if scheme != "https" && scheme != "http" {
        return Err(invalid_origin_error());
    }

    if authority.as_str().contains('@') {
        return Err(invalid_origin_error());
    }

    if let Some(path_and_query) = uri.path_and_query() {
        let path = path_and_query.path();

        if !path.is_empty() && path != "/" {
            return Err(invalid_origin_error());
        }
    }

    Ok(())
}

fn invalid_origin_error() -> ConfigError {
    ConfigError::InvalidCorsConfig {
        field: CorsConfigField::AllowOrigin,
        reason: ConfigValidationErrorReason::InvalidOrigin,
    }
}

#[cfg(test)]
#[path = "cors_tests.rs"]
mod tests;
