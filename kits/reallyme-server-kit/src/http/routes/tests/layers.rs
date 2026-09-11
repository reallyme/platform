// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use axum::Router;
use axum::routing::get;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::super::apply_standard_router_layers;
use super::fixtures::test_http_config;

#[tokio::test]
async fn standard_layers_respond_over_real_tcp_listener() {
    let app = apply_standard_router_layers(
        Router::new().route("/hello", get(|| async { "hello\n" })),
        &test_http_config(Duration::from_secs(5), 1024 * 1024),
    );
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        // Some local sandboxes deny opening TCP listeners. CI must run this
        // test with socket access so the standard layer stack is still
        // exercised over a real listener.
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(error) => panic!("test listener should bind: {error}"),
    };
    let address = listener
        .local_addr()
        .expect("test listener should expose address");
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let server_task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
    });

    let mut stream = TcpStream::connect(address)
        .await
        .expect("test client should connect");
    stream
        .write_all(b"GET /hello HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .await
        .expect("test request should write");

    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response))
        .await
        .expect("test response should complete before timeout")
        .expect("test response should read");

    let response = String::from_utf8_lossy(&response);
    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "unexpected response: {response}"
    );
    assert!(
        response.contains("hello"),
        "response body missing hello: {response}"
    );

    let _ = shutdown_tx.send(());
    tokio::time::timeout(Duration::from_secs(2), server_task)
        .await
        .expect("server task should stop")
        .expect("server task should join")
        .expect("server should exit cleanly");
}
