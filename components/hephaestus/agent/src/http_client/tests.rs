// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::shared_http_client;

#[tokio::test]
async fn shared_http_client_does_not_follow_redirects() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap_or_else(|error| panic!("listener binds: {error:?}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("listener address: {error:?}"));
    let server = tokio::spawn(async move {
        let (mut stream, _peer) = listener
            .accept()
            .await
            .unwrap_or_else(|error| panic!("accept request: {error:?}"));
        let mut request = [0u8; 512];
        let _bytes_read = stream
            .read(&mut request)
            .await
            .unwrap_or_else(|error| panic!("read request: {error:?}"));
        stream
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: http://example.invalid/\r\nContent-Length: 0\r\n\r\n",
            )
            .await
            .unwrap_or_else(|error| panic!("write response: {error:?}"));
    });

    let response = shared_http_client()
        .unwrap_or_else(|error| panic!("client builds: {error:?}"))
        .get(format!("http://{address}/"))
        .send()
        .await
        .unwrap_or_else(|error| panic!("request succeeds: {error:?}"));

    assert_eq!(response.status().as_u16(), 302);
    server
        .await
        .unwrap_or_else(|error| panic!("server joins: {error:?}"));
}
