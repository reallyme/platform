// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use secrecy::SecretString;
use std::path::PathBuf;
use temp_env::with_vars;

use super::{
    MAX_CONNECTION_URI_BYTES, MAX_TLS_CA_PATH_BYTES, PostgresConfig, PostgresConfigInput,
    PostgresTlsTrust, PostgresTransportSecurity,
};
use crate::error::{PostgresConfigErrorReason, PostgresConfigField, PostgresError};

fn valid_input() -> PostgresConfigInput {
    PostgresConfigInput {
        connection_uri: SecretString::from(
            "postgres://audit-user:credential@postgres.internal/audit",
        ),
        ..PostgresConfigInput::default()
    }
}

#[test]
fn config_rejects_empty_connection_uri() {
    let result = PostgresConfig::new(PostgresConfigInput::default());

    assert_eq!(
        result.err(),
        Some(PostgresError::Config {
            field: PostgresConfigField::ConnectionUri,
            reason: PostgresConfigErrorReason::Empty,
        })
    );
}

#[test]
fn config_rejects_oversized_connection_uri() {
    let result = PostgresConfig::new(PostgresConfigInput {
        connection_uri: SecretString::from("x".repeat(MAX_CONNECTION_URI_BYTES + 1)),
        ..PostgresConfigInput::default()
    });

    assert_eq!(
        result.err(),
        Some(PostgresError::Config {
            field: PostgresConfigField::ConnectionUri,
            reason: PostgresConfigErrorReason::TooLarge,
        })
    );
}

#[test]
fn config_rejects_zero_pool_size() {
    let result = PostgresConfig::new(PostgresConfigInput {
        connection_uri: SecretString::from("host=localhost user=postgres"),
        max_pool_size: 0,
        ..PostgresConfigInput::default()
    });

    assert_eq!(
        result.err(),
        Some(PostgresError::Config {
            field: PostgresConfigField::MaxPoolSize,
            reason: PostgresConfigErrorReason::Zero,
        })
    );
}

#[test]
fn config_rejects_minimum_pool_size_above_maximum() {
    let result = PostgresConfig::new(PostgresConfigInput {
        max_pool_size: 8,
        min_pool_size: 9,
        ..valid_input()
    });

    assert_eq!(
        result.err(),
        Some(PostgresError::Config {
            field: PostgresConfigField::MinPoolSize,
            reason: PostgresConfigErrorReason::TooLarge,
        })
    );
}

#[test]
fn config_allows_a_zero_minimum_for_explicit_lazy_test_fixtures() {
    let config = PostgresConfig::new(PostgresConfigInput {
        min_pool_size: 0,
        ..valid_input()
    })
    .expect("zero minimum remains a valid explicit pool policy");

    assert_eq!(config.min_pool_size(), 0);
}

#[test]
fn config_requires_tls_by_default() {
    let config = PostgresConfig::new(valid_input()).expect("valid fixture should parse");

    assert_eq!(
        config.transport_security(),
        PostgresTransportSecurity::RequireTls
    );
}

#[test]
fn config_debug_redacts_connection_credentials() {
    let config = PostgresConfig::new(valid_input()).expect("valid fixture should parse");
    let debug = format!("{config:?}");

    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("credential"));
    assert!(!debug.contains("audit-user"));
}

#[test]
fn config_debug_redacts_private_ca_path() {
    let config = PostgresConfig::new(PostgresConfigInput {
        tls_trust: PostgresTlsTrust::CustomRootCertificate(PathBuf::from(
            "/private/platform/postgres-root.pem",
        )),
        ..valid_input()
    })
    .expect("valid fixture should parse");
    let debug = format!("{config:?}");

    assert!(debug.contains("custom-root-certificate"));
    assert!(!debug.contains("/private/platform"));
}

#[test]
fn config_rejects_zero_statement_timeout() {
    let result = PostgresConfig::new(PostgresConfigInput {
        statement_timeout_millis: 0,
        ..valid_input()
    });

    assert_eq!(
        result.err(),
        Some(PostgresError::Config {
            field: PostgresConfigField::StatementTimeoutMillis,
            reason: PostgresConfigErrorReason::Zero,
        })
    );
}

#[test]
fn config_rejects_unsafe_application_name_characters() {
    let result = PostgresConfig::new(PostgresConfigInput {
        application_name: Some("example\nservice".to_owned()),
        ..valid_input()
    });

    assert_eq!(
        result.err(),
        Some(PostgresError::Config {
            field: PostgresConfigField::ApplicationName,
            reason: PostgresConfigErrorReason::InvalidSyntax,
        })
    );
}

#[test]
fn config_rejects_tls_trust_when_plaintext_is_selected() {
    let result = PostgresConfig::new(PostgresConfigInput {
        transport_security: PostgresTransportSecurity::AllowPlaintextForDevelopment,
        tls_trust: PostgresTlsTrust::CustomRootCertificate(PathBuf::from("postgres-ca.pem")),
        ..valid_input()
    });

    assert_eq!(
        result.err(),
        Some(PostgresError::Config {
            field: PostgresConfigField::TlsCaCertificatePath,
            reason: PostgresConfigErrorReason::Incompatible,
        })
    );
}

#[test]
fn config_rejects_oversized_tls_ca_path() {
    let result = PostgresConfig::new(PostgresConfigInput {
        tls_trust: PostgresTlsTrust::CustomRootCertificate(PathBuf::from(
            "x".repeat(MAX_TLS_CA_PATH_BYTES + 1),
        )),
        ..valid_input()
    });

    assert_eq!(
        result.err(),
        Some(PostgresError::Config {
            field: PostgresConfigField::TlsCaCertificatePath,
            reason: PostgresConfigErrorReason::TooLarge,
        })
    );
}

#[test]
fn environment_requires_an_exact_plaintext_development_opt_in() {
    with_vars(
        [
            (
                "POSTGRES_KIT_TEST_POSTGRES_URI",
                Some("postgres://localhost/test"),
            ),
            (
                "POSTGRES_KIT_TEST_POSTGRES_TLS_MODE",
                Some("allow-plaintext-development"),
            ),
        ],
        || {
            let config = PostgresConfig::from_env_prefix("POSTGRES_KIT_TEST")
                .expect("valid development fixture should parse");

            assert_eq!(
                config.transport_security(),
                PostgresTransportSecurity::AllowPlaintextForDevelopment
            );
        },
    );
}

#[test]
fn environment_rejects_unknown_tls_mode() {
    with_vars(
        [
            (
                "POSTGRES_KIT_INVALID_TLS_POSTGRES_URI",
                Some("postgres://localhost/test"),
            ),
            ("POSTGRES_KIT_INVALID_TLS_POSTGRES_TLS_MODE", Some("prefer")),
        ],
        || {
            let result = PostgresConfig::from_env_prefix("POSTGRES_KIT_INVALID_TLS");

            assert_eq!(
                result.err(),
                Some(PostgresError::Config {
                    field: PostgresConfigField::TransportSecurity,
                    reason: PostgresConfigErrorReason::InvalidSyntax,
                })
            );
        },
    );
}

#[test]
fn environment_rejects_invalid_prefix_without_entering_std_env() {
    let result = PostgresConfig::from_env_prefix("POSTGRES-KIT=INVALID");

    assert_eq!(
        result.err(),
        Some(PostgresError::Config {
            field: PostgresConfigField::EnvironmentPrefix,
            reason: PostgresConfigErrorReason::InvalidSyntax,
        })
    );
}

#[test]
fn environment_rejects_an_explicitly_empty_application_name() {
    with_vars(
        [
            (
                "POSTGRES_KIT_EMPTY_APP_POSTGRES_URI",
                Some("postgres://localhost/test"),
            ),
            ("POSTGRES_KIT_EMPTY_APP_POSTGRES_APPLICATION_NAME", Some("")),
        ],
        || {
            let result = PostgresConfig::from_env_prefix("POSTGRES_KIT_EMPTY_APP");

            assert_eq!(
                result.err(),
                Some(PostgresError::Config {
                    field: PostgresConfigField::ApplicationName,
                    reason: PostgresConfigErrorReason::Empty,
                })
            );
        },
    );
}

#[cfg(unix)]
#[test]
fn environment_rejects_non_unicode_values() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    with_vars(
        [(
            OsString::from("POSTGRES_KIT_ENCODING_POSTGRES_URI"),
            Some(OsString::from_vec(vec![0xff, 0xfe])),
        )],
        || {
            let result = PostgresConfig::from_env_prefix("POSTGRES_KIT_ENCODING");

            assert_eq!(
                result.err(),
                Some(PostgresError::Config {
                    field: PostgresConfigField::ConnectionUri,
                    reason: PostgresConfigErrorReason::InvalidEncoding,
                })
            );
        },
    );
}
