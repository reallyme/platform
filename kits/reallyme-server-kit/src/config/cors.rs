// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use super::{ExactCorsOrigin, ExactCorsOrigins};
    use crate::config::{
        ConfigError, ConfigValidationErrorReason, CorsConfig, CorsConfigField, ServiceEnvironment,
    };

    #[test]
    fn exact_origin_accepts_valid_prod_origin() {
        let origin = ExactCorsOrigin::new("https://app.reallyme.net");

        assert_eq!(
            origin.map(|origin| origin.as_str().to_owned()),
            Ok("https://app.reallyme.net".to_owned())
        );
    }

    #[test]
    fn exact_origin_accepts_valid_localhost_origin() {
        let localhost = ExactCorsOrigin::new("http://localhost:3000");
        let loopback = ExactCorsOrigin::new("http://127.0.0.1:3000");

        assert_eq!(
            localhost.map(|origin| origin.as_str().to_owned()),
            Ok("http://localhost:3000".to_owned())
        );
        assert_eq!(
            loopback.map(|origin| origin.as_str().to_owned()),
            Ok("http://127.0.0.1:3000".to_owned())
        );
    }

    #[test]
    fn exact_origin_rejects_invalid_path() {
        assert_invalid_origin("https://app.reallyme.net/app");
    }

    #[test]
    fn exact_origin_allows_empty_path_or_root_path() {
        assert!(ExactCorsOrigin::new("https://app.reallyme.net").is_ok());
        assert!(ExactCorsOrigin::new("https://app.reallyme.net/").is_ok());
    }

    #[test]
    fn exact_origin_collection_rejects_empty_lists() {
        let result = ExactCorsOrigins::new(Vec::new());

        assert_eq!(
            result,
            Err(ConfigError::InvalidCorsConfig {
                field: CorsConfigField::AllowOrigin,
                reason: ConfigValidationErrorReason::MustBeNonEmpty,
            })
        );
    }

    #[test]
    fn cors_config_accepts_multiple_exact_origins() {
        let result = CorsConfig::allow_exact_origins(vec![
            "https://app.reallyme.net".to_owned(),
            "https://admin.reallyme.net".to_owned(),
        ]);

        assert!(matches!(result, Ok(CorsConfig::ExactOrigins(_))));
    }

    #[test]
    fn exact_origin_rejects_invalid_query() {
        assert_invalid_origin("https://app.reallyme.net?debug=true");
    }

    #[test]
    fn exact_origin_rejects_invalid_fragment() {
        assert_invalid_origin("https://app.reallyme.net#fragment");
    }

    #[test]
    fn exact_origin_rejects_whitespace() {
        assert_invalid_origin("https://app.reallyme.net ");
        assert_invalid_origin("https://app.reallyme .net");
    }

    #[test]
    fn exact_origin_rejects_invalid_scheme() {
        assert_invalid_origin("ftp://app.reallyme.net");
    }

    #[test]
    fn cors_any_is_allowed_only_in_non_production_environments() {
        assert!(matches!(
            CorsConfig::allow_any_for_development_only(ServiceEnvironment::Local),
            Ok(CorsConfig::AnyForDevelopmentOnly)
        ));
        assert!(matches!(
            CorsConfig::allow_any_for_development_only(ServiceEnvironment::Dev),
            Ok(CorsConfig::AnyForDevelopmentOnly)
        ));
        assert_eq!(
            CorsConfig::allow_any_for_development_only(ServiceEnvironment::Staging),
            Err(ConfigError::CorsPolicyDisallowedInEnvironment {
                service_environment: ServiceEnvironment::Staging,
            })
        );
        assert_eq!(
            CorsConfig::allow_any_for_development_only(ServiceEnvironment::Prod),
            Err(ConfigError::CorsPolicyDisallowedInEnvironment {
                service_environment: ServiceEnvironment::Prod,
            })
        );
    }

    #[test]
    fn exact_origin_rejects_userinfo_and_arbitrary_non_origin_strings() {
        for value in [
            "https://user@app.reallyme.net",
            "not-really-an-origin",
            "not really an origin",
        ] {
            assert_invalid_origin(value);
        }
    }

    fn assert_invalid_origin(value: &str) {
        assert_eq!(
            ExactCorsOrigin::new(value),
            Err(ConfigError::InvalidCorsConfig {
                field: CorsConfigField::AllowOrigin,
                reason: ConfigValidationErrorReason::InvalidOrigin,
            })
        );
    }
}
