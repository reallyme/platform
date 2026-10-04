// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::HeaderValue;
use std::fmt;
use url::{Host, Url};

use super::environment::ServiceEnvironment;
use super::error::{ConfigError, ConfigValidationErrorReason, CorsConfigField};

/// Shared CORS policy configuration.
///
/// This type only models generic transport policy. Service-specific route or
/// product authorization policy must not be encoded here.
#[derive(Clone, PartialEq, Eq)]
pub struct CorsConfig(CorsPolicy);

#[derive(Clone, PartialEq, Eq)]
enum CorsPolicy {
    NoCors,
    ExactOrigins(ExactCorsOrigins),
    AnyForDevelopmentOnly,
}

impl CorsConfig {
    /// Disables CORS response headers.
    pub fn no_cors() -> Self {
        Self(CorsPolicy::NoCors)
    }

    /// Allows exactly one origin after validating origin syntax and header
    /// compatibility.
    pub fn allow_exact_origin(origin: &str) -> Result<Self, ConfigError> {
        Ok(Self(CorsPolicy::ExactOrigins(ExactCorsOrigins::new(
            vec![ExactCorsOrigin::new(origin)?],
        )?)))
    }

    /// Allows one or more exact origins after validating syntax and header
    /// compatibility for each configured origin.
    pub fn allow_exact_origins(origins: Vec<String>) -> Result<Self, ConfigError> {
        let validated = origins
            .into_iter()
            .map(|origin| ExactCorsOrigin::new(origin.as_str()))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self(CorsPolicy::ExactOrigins(ExactCorsOrigins::new(
            validated,
        )?)))
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

        Ok(Self(CorsPolicy::AnyForDevelopmentOnly))
    }

    /// Returns whether CORS is disabled.
    pub const fn is_disabled(&self) -> bool {
        matches!(self.0, CorsPolicy::NoCors)
    }

    /// Returns validated exact origins when this policy uses an allowlist.
    pub const fn exact_origins(&self) -> Option<&ExactCorsOrigins> {
        match &self.0 {
            CorsPolicy::ExactOrigins(origins) => Some(origins),
            CorsPolicy::NoCors | CorsPolicy::AnyForDevelopmentOnly => None,
        }
    }

    /// Returns whether the validated development policy permits every origin.
    pub const fn allows_any_origin(&self) -> bool {
        matches!(self.0, CorsPolicy::AnyForDevelopmentOnly)
    }
}

impl fmt::Debug for CorsConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            CorsPolicy::NoCors => formatter.write_str("CorsConfig::NoCors"),
            CorsPolicy::ExactOrigins(origins) => formatter
                .debug_tuple("CorsConfig::ExactOrigins")
                .field(origins)
                .finish(),
            CorsPolicy::AnyForDevelopmentOnly => {
                formatter.write_str("CorsConfig::AnyForDevelopmentOnly")
            }
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
    /// Remote origins require HTTPS. Loopback HTTP is accepted for local
    /// services, but CORS remains explicit and disabled by default.
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
    if origin.chars().any(char::is_whitespace) || origin.contains('\\') {
        return Err(invalid_origin_error());
    }
    let parsed = Url::parse(origin).map_err(|_| invalid_origin_error())?;
    if !matches!(parsed.scheme(), "https" | "http")
        || parsed.host().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
        || parsed.origin().ascii_serialization() != origin
    {
        return Err(invalid_origin_error());
    }
    let loopback = match parsed.host() {
        Some(Host::Domain(name)) => name == "localhost",
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => {
            address.is_loopback()
                || address
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| mapped.is_loopback())
        }
        None => false,
    };
    if parsed.scheme() == "http" && !loopback {
        return Err(invalid_origin_error());
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
