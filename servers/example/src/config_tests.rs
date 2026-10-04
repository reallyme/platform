// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};

use reallyme_app_kit::AppConfigProfile;
use thiserror::Error;

use super::{ExampleServerConfig, MAX_SERVER_CONFIG_BYTES};
use crate::error::ExampleServerErrorReason;

#[derive(Debug, Error)]
enum ConfigTestError {
    #[error("temporary config file operation failed")]
    File(#[from] std::io::Error),
    #[error("test fixture length is not representable")]
    FixtureLength,
}

#[test]
fn checked_in_local_config_is_valid_and_loopback_only()
-> Result<(), crate::error::ExampleServerError> {
    let config =
        ExampleServerConfig::from_jsonc_str(include_str!("../config/example-server.jsonc"))?;

    assert_eq!(config.application_profile(), AppConfigProfile::Local);
    assert_eq!(
        config.http().bind_address().ip_addr(),
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    );
    assert_eq!(config.http().bind_address().port().as_u16(), 8080);
    Ok(())
}

#[test]
fn container_config_uses_production_profile_and_exact_host()
-> Result<(), crate::error::ExampleServerError> {
    let config = ExampleServerConfig::from_jsonc_str(include_str!(
        "../deploy/example-server.container.jsonc"
    ))?;
    assert_eq!(config.application_profile(), AppConfigProfile::Prod);
    assert!(!config.http().bind_address().ip_addr().is_loopback());
    let hosts = config.http().security().host_authority_policy();
    assert!(hosts.allows("example.reallyme.net"));
    assert!(!hosts.allows("untrusted.example.net"));
    Ok(())
}

#[test]
fn non_loopback_bind_requires_explicit_allowed_hosts() {
    let result = ExampleServerConfig::from_jsonc_str(
        r#"{
            "application_profile": "prod",
            "bind_address": "0.0.0.0:8080",
            "request_timeout_seconds": 30,
            "request_body_limit_bytes": 1048576,
            "metrics_idle_timeout_seconds": 30,
            "shutdown_timeout_seconds": 10
        }"#,
    );
    assert!(matches!(
        result,
        Err(error) if error.reason() == ExampleServerErrorReason::AllowedHostsInvalid
    ));
}

#[test]
fn trusted_proxy_jsonc_selects_exact_ranges_and_header_family()
-> Result<(), crate::error::ExampleServerError> {
    let config = ExampleServerConfig::from_jsonc_str(
        r#"{
            "application_profile": "prod",
            "bind_address": "0.0.0.0:8080",
            "allowed_hosts": ["example.reallyme.net"],
            "trusted_proxy_ranges": ["127.0.0.1/32"],
            "external_origin_policy": {
                "header_family": "x_forwarded",
                "trusted_forwarded_host": true,
                "trusted_forwarded_proto": true,
                "require_https_external_scheme": true,
                "strict_forwarded_header_consistency": true,
                "strip_raw_proxy_headers": true
            },
            "request_timeout_seconds": 30,
            "request_body_limit_bytes": 1048576,
            "metrics_idle_timeout_seconds": 30,
            "shutdown_timeout_seconds": 10
        }"#,
    )?;
    let security = config.http().security();
    assert!(
        security
            .trusted_proxy_headers()
            .trusts_peer(Some(IpAddr::V4(Ipv4Addr::LOCALHOST)))
    );
    assert!(
        !security
            .trusted_proxy_headers()
            .trusts_peer(Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))))
    );
    assert_eq!(
        security.external_origin_policy().header_family(),
        reallyme_server_kit::config::TrustedProxyHeaderFamily::XForwarded
    );
    assert!(
        security
            .external_origin_policy()
            .require_https_external_scheme()
    );
    Ok(())
}

#[test]
fn listener_connection_limits_are_parsed_and_validated() {
    let valid = ExampleServerConfig::from_jsonc_str(
        r#"{
            "application_profile": "local",
            "bind_address": "127.0.0.1:8080",
            "request_timeout_seconds": 30,
            "request_body_limit_bytes": 1048576,
            "metrics_idle_timeout_seconds": 30,
            "shutdown_timeout_seconds": 10,
            "connection_limits": { "max_live": 128, "max_per_source": 32 }
        }"#,
    )
    .expect("valid TCP limits");
    assert_eq!(valid.http().connection_limits().max_live(), 128);
    assert_eq!(valid.http().connection_limits().max_per_source(), 32);

    let invalid = ExampleServerConfig::from_jsonc_str(
        r#"{
            "application_profile": "local",
            "bind_address": "127.0.0.1:8080",
            "request_timeout_seconds": 30,
            "request_body_limit_bytes": 1048576,
            "metrics_idle_timeout_seconds": 30,
            "shutdown_timeout_seconds": 10,
            "connection_limits": { "max_live": 32, "max_per_source": 64 }
        }"#,
    );
    assert!(matches!(
        invalid,
        Err(error) if error.reason() == ExampleServerErrorReason::ConnectionLimitsInvalid
    ));
}

#[test]
fn external_origin_policy_without_trusted_proxy_ranges_fails_closed() {
    let result = ExampleServerConfig::from_jsonc_str(
        r#"{
            "application_profile": "prod",
            "bind_address": "0.0.0.0:8080",
            "allowed_hosts": ["example.reallyme.net"],
            "external_origin_policy": {
                "header_family": "x_forwarded",
                "trusted_forwarded_host": true,
                "trusted_forwarded_proto": true,
                "require_https_external_scheme": true,
                "strict_forwarded_header_consistency": true,
                "strip_raw_proxy_headers": true
            },
            "request_timeout_seconds": 30,
            "request_body_limit_bytes": 1048576,
            "metrics_idle_timeout_seconds": 30,
            "shutdown_timeout_seconds": 10
        }"#,
    );
    assert!(matches!(
        result,
        Err(error) if error.reason() == ExampleServerErrorReason::ExternalOriginPolicyInvalid
    ));
}

#[test]
fn invalid_trusted_proxy_range_is_rejected_at_jsonc_boundary() {
    let result = ExampleServerConfig::from_jsonc_str(
        r#"{
            "application_profile": "prod",
            "bind_address": "0.0.0.0:8080",
            "allowed_hosts": ["example.reallyme.net"],
            "trusted_proxy_ranges": ["0.0.0.0/0"],
            "request_timeout_seconds": 30,
            "request_body_limit_bytes": 1048576,
            "metrics_idle_timeout_seconds": 30,
            "shutdown_timeout_seconds": 10
        }"#,
    );
    assert!(matches!(
        result,
        Err(error) if error.reason() == ExampleServerErrorReason::TrustedProxyRangesInvalid
    ));
}

#[test]
fn unknown_fields_are_rejected() {
    let result = ExampleServerConfig::from_jsonc_str(
        r#"{
            "application_profile": "local",
            "bind_address": "127.0.0.1:8080",
            "request_timeout_seconds": 30,
            "request_body_limit_bytes": 1048576,
            "metrics_idle_timeout_seconds": 30,
            "shutdown_timeout_seconds": 10,
            "unexpected": true
        }"#,
    );

    assert!(matches!(
        result,
        Err(error)
            if error.reason() == ExampleServerErrorReason::ServerConfigDocumentInvalid
    ));
}

#[test]
fn zero_request_timeout_is_rejected() {
    let result = ExampleServerConfig::from_jsonc_str(
        r#"{
            "application_profile": "local",
            "bind_address": "127.0.0.1:8080",
            "request_timeout_seconds": 0,
            "request_body_limit_bytes": 1048576,
            "metrics_idle_timeout_seconds": 30,
            "shutdown_timeout_seconds": 10
        }"#,
    );

    assert!(matches!(
        result,
        Err(error) if error.reason() == ExampleServerErrorReason::RequestTimeoutInvalid
    ));
}

#[test]
fn non_utf8_config_file_is_rejected() -> Result<(), ConfigTestError> {
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(&[0xff])?;

    let result = ExampleServerConfig::load(file.path());

    assert!(matches!(
        result,
        Err(error) if error.reason() == ExampleServerErrorReason::ServerConfigFileNotUtf8
    ));
    Ok(())
}

#[test]
fn oversized_config_file_is_rejected_before_parsing() -> Result<(), ConfigTestError> {
    let oversized_length = MAX_SERVER_CONFIG_BYTES
        .checked_add(1)
        .and_then(|length| usize::try_from(length).ok())
        .ok_or(ConfigTestError::FixtureLength)?;
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(vec![b' '; oversized_length].as_slice())?;

    let result = ExampleServerConfig::load(file.path());

    assert!(matches!(
        result,
        Err(error) if error.reason() == ExampleServerErrorReason::ServerConfigFileTooLarge
    ));
    Ok(())
}
