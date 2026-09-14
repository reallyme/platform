// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use secrecy::SecretString;
use tempfile::tempdir;

use super::{MAX_TLS_CA_PEM_BYTES, PostgresTransportSecurity, configured_client, tls_connector};
use crate::config::{PostgresConfig, PostgresConfigInput, PostgresTlsTrust};
use crate::error::{PostgresError, PostgresSetupErrorReason};
use tokio_postgres::config::SslMode;

fn config(transport_security: PostgresTransportSecurity) -> PostgresConfig {
    PostgresConfig::new(PostgresConfigInput {
        connection_uri: SecretString::from(
            "postgres://audit:credential@postgres.internal/audit?sslmode=prefer",
        ),
        transport_security,
        ..PostgresConfigInput::default()
    })
    .expect("valid fixture should parse")
}

#[test]
fn configured_client_forces_required_tls() {
    let value = config(PostgresTransportSecurity::RequireTls);
    let client = configured_client(&value).expect("valid fixture should configure");

    assert_eq!(client.get_ssl_mode(), SslMode::Require);
}

#[test]
fn configured_client_forces_disabled_tls_only_for_development() {
    let value = config(PostgresTransportSecurity::AllowPlaintextForDevelopment);
    let client = configured_client(&value).expect("valid fixture should configure");

    assert_eq!(client.get_ssl_mode(), SslMode::Disable);
}

#[test]
fn custom_tls_trust_rejects_malformed_pem() {
    let directory = tempdir().expect("fixture directory should be created");
    let path = directory.path().join("postgres-ca.pem");
    std::fs::write(&path, b"not-a-certificate").expect("malformed fixture should be written");
    let config = PostgresConfig::new(PostgresConfigInput {
        connection_uri: SecretString::from("postgres://postgres.internal/audit"),
        tls_trust: PostgresTlsTrust::CustomRootCertificate(path),
        ..PostgresConfigInput::default()
    })
    .expect("valid fixture should parse");

    assert!(matches!(
        tls_connector(&config),
        Err(PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustInvalid,
        })
    ));
}

#[test]
fn custom_tls_trust_rejects_oversized_files_before_parsing() {
    let directory = tempdir().expect("fixture directory should be created");
    let path = directory.path().join("oversized-postgres-ca.pem");
    let file = std::fs::File::create(&path).expect("fixture should be created");
    file.set_len(MAX_TLS_CA_PEM_BYTES + 1)
        .expect("fixture length should be set");
    let config = PostgresConfig::new(PostgresConfigInput {
        connection_uri: SecretString::from("postgres://postgres.internal/audit"),
        tls_trust: PostgresTlsTrust::CustomRootCertificate(path),
        ..PostgresConfigInput::default()
    })
    .expect("valid fixture should parse");

    assert!(matches!(
        tls_connector(&config),
        Err(PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustTooLarge,
        })
    ));
}
