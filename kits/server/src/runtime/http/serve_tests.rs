// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::Router;
use axum::extract::ConnectInfo;
use axum::http::StatusCode;
use axum::routing::get;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::serve_http;
use crate::config::HttpHeaderLimitConfig;
use crate::http::HttpListenerName;
use crate::shutdown::ShutdownReason;
use crate::task::ShutdownController;

#[tokio::test]
async fn owned_listener_preserves_peer_identity_and_serves_http() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener has an address");
    let controller = ShutdownController::new();
    let router = Router::new().route(
        "/peer",
        get(|ConnectInfo(peer): ConnectInfo<SocketAddr>| async move { peer.ip().to_string() }),
    );
    let task = tokio::spawn(serve_http(
        listener,
        router,
        HttpListenerName::new("test-http").expect("test listener name"),
        HttpHeaderLimitConfig::secure_defaults(),
        controller.token(),
    ));

    let mut client = TcpStream::connect(address)
        .await
        .expect("client should connect");
    client
        .write_all(b"GET /peer HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("request should write");
    let mut response = Vec::new();
    timeout(Duration::from_secs(2), client.read_to_end(&mut response))
        .await
        .expect("response should finish")
        .expect("response should read");
    let response = String::from_utf8(response).expect("response should be UTF-8");
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("127.0.0.1"));

    controller.begin_shutdown(ShutdownReason::Sigterm);
    assert!(task.await.expect("listener task should join").is_ok());
}

struct DropMarker(Arc<AtomicBool>);

impl Drop for DropMarker {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn aborting_listener_aborts_its_active_connection_tasks() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener has an address");
    let controller = ShutdownController::new();
    let dropped = Arc::new(AtomicBool::new(false));
    let (started_tx, mut started_rx) = mpsc::channel(1);
    let router = Router::new().route(
        "/slow",
        get({
            let dropped = Arc::clone(&dropped);
            move || {
                let dropped = Arc::clone(&dropped);
                let started_tx = started_tx.clone();
                async move {
                    let _marker = DropMarker(dropped);
                    started_tx.send(()).await.expect("start signal should send");
                    std::future::pending::<()>().await;
                    StatusCode::OK
                }
            }
        }),
    );
    let task = tokio::spawn(serve_http(
        listener,
        router,
        HttpListenerName::new("test-http").expect("test listener name"),
        HttpHeaderLimitConfig::secure_defaults(),
        controller.token(),
    ));

    let mut client = TcpStream::connect(address)
        .await
        .expect("client should connect");
    client
        .write_all(b"GET /slow HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("request should write");
    timeout(Duration::from_secs(2), started_rx.recv())
        .await
        .expect("slow handler should start")
        .expect("start signal should be present");

    controller.begin_shutdown(ShutdownReason::Sigterm);
    task.abort();
    let _ = task.await;
    timeout(Duration::from_secs(2), async {
        while !dropped.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("active handler should be dropped with listener task");
}

#[tokio::test]
async fn completed_http2_preface_without_a_request_releases_connection_capacity() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener has an address");
    let controller = ShutdownController::new();
    let task = tokio::spawn(serve_http(
        listener,
        Router::new(),
        HttpListenerName::new("test-http").expect("test listener name"),
        HttpHeaderLimitConfig::secure_defaults(),
        controller.token(),
    ));

    let mut client = TcpStream::connect(address)
        .await
        .expect("client should connect");
    // HTTP/2 connection preface followed by an empty SETTINGS frame. A
    // completed preface alone must not count as the first routed request.
    client
        .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n\0\0\0\x04\0\0\0\0\0")
        .await
        .expect("preface should write");
    let mut response = Vec::new();
    timeout(Duration::from_secs(7), client.read_to_end(&mut response))
        .await
        .expect("idle preface should be closed by the first-request deadline")
        .expect("socket should close cleanly");

    controller.begin_shutdown(ShutdownReason::Sigterm);
    assert!(task.await.expect("listener task should join").is_ok());
}
