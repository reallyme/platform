// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::http_client_builder;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn signed_transport_never_follows_redirects() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let address = listener.local_addr().expect("address");
    let client = http_client_builder()
        .https_only(false)
        .no_proxy()
        .build()
        .expect("client");
    let serve = async {
        let (mut stream, _) = listener.accept().await.expect("request");
        let mut buffer = [0u8; 4096];
        let _request_length = stream.read(&mut buffer).await.expect("read request");
        let response = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{address}/other\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("response");
    };
    let request = async {
        let response = client
            .get(format!("http://{address}/object"))
            .send()
            .await
            .expect("response");
        assert_eq!(response.status(), 307);
    };
    tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(serve, request);
    })
    .await
    .expect("bounded test");
}

#[tokio::test]
async fn signed_transport_rejects_plaintext() {
    let client = http_client_builder().build().expect("client");
    let error = client
        .get("http://127.0.0.1/object")
        .send()
        .await
        .expect_err("plaintext URL must be rejected before network I/O");
    assert!(
        error.is_builder(),
        "a connection failure would not prove HTTPS enforcement"
    );
}

#[tokio::test]
async fn object_bytes_are_not_automatically_decompressed() {
    let compressed: &[u8] = &[
        31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 43, 46, 201, 47, 74, 77, 81, 200, 79, 202, 74, 77, 46,
        81, 72, 170, 44, 73, 45, 6, 0, 231, 129, 92, 205, 19, 0, 0, 0,
    ];
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let address = listener.local_addr().expect("address");
    let client = http_client_builder()
        .https_only(false)
        .no_proxy()
        .build()
        .expect("client");
    let serve = async {
        let (mut stream, _) = listener.accept().await.expect("request");
        let mut buffer = [0u8; 4096];
        let received = stream.read(&mut buffer).await.expect("read");
        assert!(
            received > 0,
            "request must arrive before the fixture responds"
        );
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            compressed.len()
        );
        stream.write_all(headers.as_bytes()).await.expect("headers");
        stream.write_all(compressed).await.expect("body");
    };
    let request = async {
        let response = client
            .get(format!("http://{address}/object"))
            .send()
            .await
            .expect("response");
        assert_eq!(response.bytes().await.expect("bytes").as_ref(), compressed);
    };
    tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(serve, request);
    })
    .await
    .expect("bounded test");
}
