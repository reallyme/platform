// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use secrecy::SecretString;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn plaintext_seed_does_not_follow_discovered_server_with_token() {
    let seed = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("seed listener binds");
    let discovered = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("discovered listener binds");
    let seed_address = seed.local_addr().expect("seed address");
    let discovered_address = discovered.local_addr().expect("discovered address");
    let seed_task = tokio::spawn(async move {
        let (mut stream, _) = seed.accept().await.expect("seed accepts client");
        let info = format!(
            "INFO {{\"server_id\":\"seed\",\"server_name\":\"seed\",\"version\":\"2.10.0\",\"proto\":1,\"host\":\"127.0.0.1\",\"port\":{},\"max_payload\":1048576,\"connect_urls\":[\"{discovered_address}\"]}}\r\n",
            seed_address.port()
        );
        stream.write_all(info.as_bytes()).await.expect("send INFO");
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 512];
        loop {
            let count = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buffer))
                .await
                .expect("client handshake deadline")
                .expect("read client handshake");
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..count]);
            if bytes.windows(6).any(|window| window == b"PING\r\n") {
                stream.write_all(b"PONG\r\n").await.expect("send PONG");
                break;
            }
        }
    });
    let credential =
        super::JetStreamCredentials::Token(SecretString::new("test-only-token".to_owned().into()));
    let client = tokio::time::timeout(
        Duration::from_secs(5),
        super::connect_with_credentials(
            &format!("nats://{seed_address}"),
            super::JetStreamTlsPolicy::Disabled,
            &credential,
        ),
    )
    .await
    .expect("client connection deadline")
    .expect("seed connection succeeds");
    seed_task.await.expect("seed task completes");
    assert!(
        tokio::time::timeout(Duration::from_secs(3), discovered.accept())
            .await
            .is_err(),
        "plaintext reconnect must not follow advertised servers"
    );
    drop(client);
}
