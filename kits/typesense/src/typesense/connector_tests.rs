// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::TypesenseConnector;
use crate::typesense::{TypesenseConfig, TypesenseEndpoint, TypesenseError};
use secrecy::SecretString;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn excessive_retry_after_returns_rate_limit_without_retrying_early() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let address = listener.local_addr().expect("address");
    let config = TypesenseConfig::new(
        TypesenseEndpoint::parse(format!("http://{address}")).expect("endpoint"),
        SecretString::from("key"),
        Duration::from_secs(1),
    )
    .expect("config")
    .with_max_retries(2)
    .with_retry_max_delay(Duration::from_millis(5));
    let connector = TypesenseConnector::connect(config).expect("connector");
    let serve = async {
        let (mut stream, _) = listener.accept().await.expect("request");
        let mut buffer = [0u8; 4096];
        let received = stream.read(&mut buffer).await.expect("read");
        assert!(
            received > 0,
            "request must arrive before the fixture responds"
        );
        stream.write_all(b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 18446744073709551615\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await.expect("response");
    };
    let request = async {
        assert!(matches!(connector.health_report().await,
            Err(TypesenseError::Upstream { status, .. }) if status == 429));
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::join!(serve, request);
    })
    .await
    .expect("bounded response");
    assert!(
        tokio::time::timeout(Duration::from_millis(30), listener.accept())
            .await
            .is_err(),
        "must not issue a premature retry"
    );
}

#[tokio::test]
async fn redirects_cannot_forward_the_typesense_api_key() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let destination = TcpListener::bind("127.0.0.1:0").await.expect("destination");
    let address = listener.local_addr().expect("address");
    let target = destination.local_addr().expect("target");
    let config = TypesenseConfig::new(
        TypesenseEndpoint::parse(format!("http://{address}")).expect("endpoint"),
        SecretString::from("secret-test-api-key"),
        Duration::from_secs(1),
    )
    .expect("config")
    .with_max_retries(0);
    let connector = TypesenseConnector::connect(config).expect("connector");
    let serve = async {
        let (mut stream, _) = listener.accept().await.expect("request");
        let mut buffer = [0u8; 4096];
        let length = stream.read(&mut buffer).await.expect("read");
        assert!(
            std::str::from_utf8(&buffer[..length])
                .expect("headers")
                .contains("secret-test-api-key")
        );
        let response = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{target}/health\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("response");
    };
    let request = async {
        assert!(matches!(connector.health_report().await,
            Err(TypesenseError::Upstream { status, .. }) if status == 307));
    };
    tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(serve, request);
    })
    .await
    .expect("bounded test");
    assert!(
        tokio::time::timeout(Duration::from_millis(20), destination.accept())
            .await
            .is_err()
    );
}
