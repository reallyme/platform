// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    MAX_TLS_CA_PEM_BYTES, decode_bounded_get_result, decode_deleted_count, read_custom_root,
};
use crate::{
    ValkeyCommandErrorReason, ValkeyConfig, ValkeyConfigInput, ValkeyDataErrorReason,
    ValkeyDataKind, ValkeyError, ValkeySetupErrorReason,
};

#[tokio::test]
async fn default_tls_connection_initializes_a_crypto_provider() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("ephemeral test port should be available");
    let port = listener.local_addr().expect("test listener address").port();
    drop(listener);
    let config = ValkeyConfig::new(ValkeyConfigInput {
        host: "127.0.0.1".to_owned(),
        port,
        connection_timeout_millis: 100,
        response_timeout_millis: 100,
        retry_attempts: 0,
        ..ValkeyConfigInput::default()
    })
    .expect("test TLS configuration should validate");

    let result = super::ValkeyConnector::connect(&config).await;
    assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    assert!(matches!(
        result,
        Err(ValkeyError::Setup {
            reason: ValkeySetupErrorReason::ConnectionUnavailable,
        })
    ));
}

#[test]
fn bounded_get_decodes_missing_empty_and_oversized_values() {
    assert!(matches!(
        decode_bounded_get_result((0, Vec::new())),
        Ok(None)
    ));
    assert!(matches!(
        decode_bounded_get_result((1, Vec::new())),
        Ok(Some(value)) if value.as_bytes().is_empty()
    ));
    assert!(matches!(
        decode_bounded_get_result((2, Vec::new())),
        Err(ValkeyError::InvalidData {
            kind: ValkeyDataKind::Value,
            reason: ValkeyDataErrorReason::TooLarge,
        })
    ));
    assert!(matches!(
        decode_bounded_get_result((0, vec![1])),
        Err(ValkeyError::Command {
            reason: ValkeyCommandErrorReason::InvalidResponse,
        })
    ));
}

#[test]
fn delete_count_rejects_protocol_violation() {
    assert_eq!(decode_deleted_count(0), Ok(false));
    assert_eq!(decode_deleted_count(1), Ok(true));
    assert_eq!(
        decode_deleted_count(2),
        Err(ValkeyError::Command {
            reason: ValkeyCommandErrorReason::InvalidResponse,
        })
    );
}

#[test]
fn custom_root_reader_rejects_missing_empty_and_oversized_files() {
    let directory = tempfile::tempdir().expect("temporary directory should be available");
    let missing = directory.path().join("missing.pem");
    assert_eq!(
        read_custom_root(&missing).err(),
        Some(ValkeyError::Setup {
            reason: ValkeySetupErrorReason::TlsTrustUnavailable,
        })
    );

    let empty = directory.path().join("empty.pem");
    std::fs::write(&empty, []).expect("empty fixture should be written");
    assert_eq!(
        read_custom_root(&empty).err(),
        Some(ValkeyError::Setup {
            reason: ValkeySetupErrorReason::TlsTrustInvalid,
        })
    );

    let oversized = directory.path().join("oversized.pem");
    let file = std::fs::File::create(&oversized).expect("oversized fixture should be created");
    file.set_len(MAX_TLS_CA_PEM_BYTES + 1)
        .expect("oversized fixture should be sized");
    assert_eq!(
        read_custom_root(&oversized).err(),
        Some(ValkeyError::Setup {
            reason: ValkeySetupErrorReason::TlsTrustTooLarge,
        })
    );
}
