// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::time::Duration;

use secrecy::SecretString;

use super::{
    MAX_CREDENTIAL_BYTES, ValkeyConfig, ValkeyConfigInput, ValkeyTlsTrust, ValkeyTransportSecurity,
};
use crate::{ValkeyConfigErrorReason, ValkeyConfigField, ValkeyError};
use temp_env::with_vars;

fn input() -> ValkeyConfigInput {
    ValkeyConfigInput {
        host: "valkey.internal".to_owned(),
        key_prefix: "reallyme:test".to_owned(),
        transport_security: ValkeyTransportSecurity::RequireTls,
        ..ValkeyConfigInput::default()
    }
}

#[test]
fn valid_configuration_is_bounded_and_tls_explicit() {
    let config = ValkeyConfig::new(input()).expect("configuration fixture should be valid");
    assert_eq!(config.host(), "valkey.internal");
    assert_eq!(
        config.transport_security(),
        ValkeyTransportSecurity::RequireTls
    );
    assert!(config.response_timeout().as_millis() > 0);
}

#[test]
fn host_rejects_uri_and_path_syntax() {
    let mut value = input();
    value.host = "rediss://valkey.internal/0".to_owned();
    assert!(matches!(
        ValkeyConfig::new(value),
        Err(ValkeyError::Config {
            field: ValkeyConfigField::Host,
            reason: ValkeyConfigErrorReason::InvalidSyntax,
        })
    ));
}

#[test]
fn queue_and_concurrency_bounds_reject_zero() {
    let mut value = input();
    value.pipeline_buffer_size = 0;
    assert!(matches!(
        ValkeyConfig::new(value),
        Err(ValkeyError::Config {
            field: ValkeyConfigField::PipelineBufferSize,
            reason: ValkeyConfigErrorReason::MustBePositive,
        })
    ));
}

#[test]
fn port_and_empty_credentials_are_rejected() {
    let mut zero_port = input();
    zero_port.port = 0;
    assert!(matches!(
        ValkeyConfig::new(zero_port),
        Err(ValkeyError::Config {
            field: ValkeyConfigField::Port,
            reason: ValkeyConfigErrorReason::MustBePositive,
        })
    ));

    let mut empty_password = input();
    empty_password.password = Some(secrecy::SecretString::from(String::new()));
    assert!(matches!(
        ValkeyConfig::new(empty_password),
        Err(ValkeyError::Config {
            field: ValkeyConfigField::Password,
            reason: ValkeyConfigErrorReason::Empty,
        })
    ));
}

#[test]
fn environment_loads_all_generic_connector_settings() {
    with_vars(
        [
            ("VALKEY_KIT_TEST_VALKEY_HOST", Some("127.0.0.1")),
            ("VALKEY_KIT_TEST_VALKEY_PORT", Some("6379")),
            ("VALKEY_KIT_TEST_VALKEY_DATABASE", Some("7")),
            ("VALKEY_KIT_TEST_VALKEY_USERNAME", Some("service")),
            ("VALKEY_KIT_TEST_VALKEY_PASSWORD", Some("credential")),
            ("VALKEY_KIT_TEST_VALKEY_KEY_PREFIX", Some("app:test")),
            (
                "VALKEY_KIT_TEST_VALKEY_TLS_MODE",
                Some("allow-plaintext-development"),
            ),
            (
                "VALKEY_KIT_TEST_VALKEY_CONNECTION_TIMEOUT_MILLIS",
                Some("4000"),
            ),
            (
                "VALKEY_KIT_TEST_VALKEY_RESPONSE_TIMEOUT_MILLIS",
                Some("3000"),
            ),
            ("VALKEY_KIT_TEST_VALKEY_RETRY_ATTEMPTS", Some("4")),
            ("VALKEY_KIT_TEST_VALKEY_CONCURRENCY_LIMIT", Some("128")),
            ("VALKEY_KIT_TEST_VALKEY_PIPELINE_BUFFER_SIZE", Some("64")),
        ],
        || {
            let config = ValkeyConfig::from_env_prefix("VALKEY_KIT_TEST")
                .expect("complete environment fixture should validate");
            assert_eq!(config.host(), "127.0.0.1");
            assert_eq!(config.port(), 6_379);
            assert_eq!(config.database(), 7);
            assert_eq!(config.key_prefix(), "app:test");
            assert_eq!(config.connection_timeout(), Duration::from_secs(4));
            assert_eq!(config.response_timeout(), Duration::from_secs(3));
            assert_eq!(config.retry_attempts(), 4);
            assert_eq!(config.concurrency_limit(), 128);
            assert_eq!(config.pipeline_buffer_size(), 64);
            assert_eq!(
                config.transport_security(),
                ValkeyTransportSecurity::AllowPlaintextForDevelopment
            );
        },
    );
}

#[test]
fn environment_requires_host_and_exact_tls_mode() {
    with_vars(
        [
            ("VALKEY_KIT_MISSING_VALKEY_HOST", None),
            ("VALKEY_KIT_MISSING_VALKEY_TLS_MODE", Some("prefer")),
        ],
        || {
            assert_eq!(
                ValkeyConfig::from_env_prefix("VALKEY_KIT_MISSING").err(),
                Some(ValkeyError::Config {
                    field: ValkeyConfigField::Host,
                    reason: ValkeyConfigErrorReason::Empty,
                })
            );
        },
    );

    with_vars(
        [
            ("VALKEY_KIT_TLS_VALKEY_HOST", Some("valkey.internal")),
            ("VALKEY_KIT_TLS_VALKEY_TLS_MODE", Some("prefer")),
        ],
        || {
            assert_eq!(
                ValkeyConfig::from_env_prefix("VALKEY_KIT_TLS").err(),
                Some(ValkeyError::Config {
                    field: ValkeyConfigField::TransportSecurity,
                    reason: ValkeyConfigErrorReason::InvalidSyntax,
                })
            );
        },
    );
}

#[test]
fn environment_rejects_invalid_prefix_before_lookup() {
    assert_eq!(
        ValkeyConfig::from_env_prefix("VALKEY-KIT=INVALID").err(),
        Some(ValkeyError::Config {
            field: ValkeyConfigField::EnvironmentPrefix,
            reason: ValkeyConfigErrorReason::InvalidSyntax,
        })
    );
}

#[test]
fn oversized_credentials_are_rejected_without_exposing_them() {
    let mut value = input();
    value.password = Some(SecretString::from("x".repeat(MAX_CREDENTIAL_BYTES + 1)));
    assert_eq!(
        ValkeyConfig::new(value).err(),
        Some(ValkeyError::Config {
            field: ValkeyConfigField::Password,
            reason: ValkeyConfigErrorReason::TooLarge,
        })
    );
}

#[test]
fn private_ca_path_is_redacted_and_conflicts_with_plaintext() {
    let private_path = "/private/platform/valkey-root.pem";
    let config = ValkeyConfig::new(ValkeyConfigInput {
        tls_trust: ValkeyTlsTrust::CustomRootCertificate(PathBuf::from(private_path)),
        ..input()
    })
    .expect("private CA fixture should validate");
    let debug = format!("{config:?}");
    assert!(debug.contains("custom-root-certificate"));
    assert!(!debug.contains(private_path));

    assert_eq!(
        ValkeyConfig::new(ValkeyConfigInput {
            transport_security: ValkeyTransportSecurity::AllowPlaintextForDevelopment,
            tls_trust: ValkeyTlsTrust::CustomRootCertificate(PathBuf::from("valkey-ca.pem")),
            ..input()
        })
        .err(),
        Some(ValkeyError::Config {
            field: ValkeyConfigField::TlsCaCertificatePath,
            reason: ValkeyConfigErrorReason::Incompatible,
        })
    );
}
