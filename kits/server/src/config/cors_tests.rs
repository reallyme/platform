// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
