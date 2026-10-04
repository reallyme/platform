// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[test]
fn explicit_ring_provider_builds_with_reqwest_kit_in_graph() {
    let _ = std::any::type_name::<reallyme_typesense_kit::TypesenseConnector>();
    let result = super::build_nats_tls_config(rustls::RootCertStore::empty());
    assert!(result.is_ok());
}

#[tokio::test]
async fn tls_required_info_reaches_handshake_without_provider_panic() {
    let _ = std::any::type_name::<reallyme_typesense_kit::TypesenseConnector>();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fake NATS listener should bind");
    let address = listener
        .local_addr()
        .expect("fake NATS listener should have an address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("client should connect");
        stream
            .write_all(b"INFO {\"tls_required\":true}\r\n")
            .await
            .expect("fake NATS INFO should be sent");
        let mut client_hello = [0_u8; 256];
        let bytes_read =
            tokio::time::timeout(Duration::from_secs(5), stream.read(&mut client_hello))
                .await
                .expect("TLS client hello should arrive")
                .expect("TLS client hello should be readable");
        assert!(bytes_read > 0, "TLS client hello must not be empty");
        assert_eq!(client_hello[0], 22, "client should begin a TLS handshake");
    });

    let url = format!("tls://{address}");
    let result = tokio::time::timeout(Duration::from_secs(10), super::connect_client(&url))
        .await
        .expect("failed TLS handshake should return promptly");
    assert!(matches!(result, Err(super::JetStreamError::ConnectFailed)));
    server.await.expect("fake NATS server should complete");
}
