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
