// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

// This file is compiled only for tests; fixture setup failures must fail the
// conformance test immediately and cannot occur in production paths.
#![allow(clippy::expect_used)]

use std::time::Duration;

use reallyme_nats_kit::config::connect_client;
use reallyme_typesense_kit::{TypesenseConfig, TypesenseConnector, TypesenseEndpoint};
use secrecy::SecretString;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn both_tls_clients_build_with_ambiguous_rustls_provider_features() {
    assert!(rustls::crypto::CryptoProvider::get_default().is_none());
    let endpoint =
        TypesenseEndpoint::parse("https://example.com").expect("fixture endpoint is valid");
    let config = TypesenseConfig::new(
        endpoint,
        SecretString::from("test-api-key"),
        Duration::from_secs(2),
    )
    .expect("fixture config is valid");
    assert!(TypesenseConnector::connect(config).is_ok());

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fake NATS listener binds");
    let address = listener.local_addr().expect("fake NATS listener address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("NATS client connects");
        stream
            .write_all(b"INFO {\"tls_required\":true}\r\n")
            .await
            .expect("NATS INFO writes");
        let mut hello = [0_u8; 256];
        let size = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut hello))
            .await
            .expect("TLS handshake begins in time")
            .expect("TLS client hello reads");
        assert!(size > 0);
        assert_eq!(hello[0], 22);
    });
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        connect_client(&format!("tls://{address}")),
    )
    .await
    .expect("failed TLS handshake returns promptly");
    assert!(result.is_err());
    server.await.expect("fake NATS task completes");
}
