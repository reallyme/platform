// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{Router, body::Body};
use futures_util::stream;
#[cfg(feature = "websocket")]
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::{HttpServePolicy, serve_http};
use crate::config::{
    ConnectionLimitConfig, HttpHeaderLimitConfig, TrustedProxyHeaders, TrustedProxyRange,
};
use crate::http::HttpListenerName;
use crate::shutdown::ShutdownReason;
use crate::task::ShutdownController;

const MAX_HTTP_CONNECTION_AGE: Duration = Duration::from_secs(600);
const CONNECTION_DRAIN_GRACE: Duration = Duration::from_secs(30);
const HTTP_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const HTTP_WRITE_STALL_TIMEOUT: Duration = Duration::from_secs(30);

fn test_policy(trusted_proxies: TrustedProxyHeaders) -> HttpServePolicy {
    HttpServePolicy {
        listener_name: HttpListenerName::new("test-http").expect("valid listener name"),
        header_limits: HttpHeaderLimitConfig::secure_defaults(),
        connection_limits: ConnectionLimitConfig::secure_defaults(),
        trusted_proxies,
        max_connection_age: MAX_HTTP_CONNECTION_AGE,
        connection_drain_grace: CONNECTION_DRAIN_GRACE,
        idle_timeout: HTTP_IDLE_TIMEOUT,
        write_stall_timeout: HTTP_WRITE_STALL_TIMEOUT,
        shutdown_drain_budget: Duration::from_secs(1),
    }
}

#[cfg(feature = "websocket")]
#[tokio::test]
async fn websocket_upgrade_survives_http_connection_age() {
    use axum::extract::ws::{Message, WebSocketUpgrade};
    use tokio_tungstenite::{connect_async, tungstenite::Message as ClientMessage};

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener address");
    let controller = ShutdownController::new();
    let router = Router::new().route(
        "/ws",
        get(|upgrade: WebSocketUpgrade| async move {
            upgrade.on_upgrade(|mut socket| async move {
                if let Some(Ok(Message::Text(text))) = socket.recv().await {
                    let _ = socket.send(Message::Text(text)).await;
                }
            })
        }),
    );
    let mut policy = test_policy(TrustedProxyHeaders::ignore_all());
    policy.max_connection_age = Duration::from_secs(1);
    policy.connection_drain_grace = Duration::from_millis(100);
    let task = tokio::spawn(serve_http(listener, router, policy, controller.token()));

    let (mut websocket, _) = connect_async(format!("ws://{address}/ws"))
        .await
        .expect("upgrade should succeed");
    tokio::time::sleep(Duration::from_millis(1_300)).await;
    websocket
        .send(ClientMessage::Text("after-age".into()))
        .await
        .expect("upgraded connection should remain writable");
    let response = timeout(Duration::from_secs(2), websocket.next())
        .await
        .expect("websocket response deadline")
        .expect("websocket response frame")
        .expect("valid websocket frame");
    assert_eq!(
        response.into_text().expect("text echo").as_str(),
        "after-age"
    );
    controller.begin_shutdown(ShutdownReason::Sigterm);
    assert!(task.await.expect("listener joins").is_ok());
}

#[cfg(feature = "websocket")]
#[tokio::test]
async fn http2_websocket_survives_http_idle_and_short_connection_age() {
    use axum::extract::ws::{Message, WebSocketUpgrade};
    use futures_util::{SinkExt, StreamExt};
    use hyper::client::conn::http2;
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use tokio_tungstenite::{WebSocketStream, tungstenite};

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener binds");
    let address = listener.local_addr().expect("listener address");
    let controller = ShutdownController::new();
    let router = Router::new().route(
        "/ws",
        axum::routing::any(|upgrade: WebSocketUpgrade| async move {
            upgrade.on_upgrade(|mut socket| async move {
                while let Some(Ok(Message::Text(text))) = socket.recv().await {
                    if socket.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
            })
        }),
    );
    let mut policy = test_policy(TrustedProxyHeaders::ignore_all());
    policy.idle_timeout = Duration::from_secs(1);
    policy.write_stall_timeout = Duration::from_secs(1);
    policy.connection_drain_grace = Duration::from_millis(250);
    policy.max_connection_age = Duration::from_millis(300);
    let server = tokio::spawn(serve_http(listener, router, policy, controller.token()));

    let io = TokioIo::new(TcpStream::connect(address).await.expect("client connects"));
    let (mut sender, connection) = http2::Builder::new(TokioExecutor::new())
        .handshake(io)
        .await
        .expect("h2 handshake");
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert!(connection.is_extended_connect_protocol_enabled());
    let driver = tokio::spawn(connection);
    let request = Request::builder()
        .method(axum::http::Method::CONNECT)
        .extension(hyper::ext::Protocol::from_static("websocket"))
        .uri("/ws")
        .header("sec-websocket-version", "13")
        .header("host", "localhost")
        .body(Body::empty())
        .expect("valid upgrade request");
    let mut response = sender
        .send_request(request)
        .await
        .expect("upgrade response");
    assert_eq!(response.status(), StatusCode::OK);
    let upgraded = hyper::upgrade::on(&mut response)
        .await
        .expect("extended connect upgrades");
    let mut socket = WebSocketStream::from_raw_socket(
        TokioIo::new(upgraded),
        tungstenite::protocol::Role::Client,
        None,
    )
    .await;
    // Repeated traffic spans both deadlines while leaving enough scheduling
    // margin for the test to distinguish idle retirement from executor load.
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        socket
            .send(tungstenite::Message::Text("still-open".into()))
            .await
            .expect("socket remains writable");
        let frame = timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("echo deadline")
            .expect("echo frame")
            .expect("valid echo");
        assert_eq!(frame.into_text().expect("text frame"), "still-open");
    }
    drop(socket);
    controller.begin_shutdown(ShutdownReason::Sigterm);
    assert!(server.await.expect("server joins").is_ok());
    driver.abort();
}

#[tokio::test]
async fn http2_idle_timeout_preserves_an_active_stream_then_retires_the_connection() {
    use hyper::client::conn::http2;
    use hyper_util::rt::{TokioExecutor, TokioIo};

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener address");
    let controller = ShutdownController::new();
    let router = Router::new()
        .route(
            "/stream",
            get(|| async { Body::from_stream(stream::pending::<Result<Bytes, std::io::Error>>()) }),
        )
        .route("/ready", get(|| async { StatusCode::OK }));
    let mut policy = test_policy(TrustedProxyHeaders::ignore_all());
    policy.idle_timeout = Duration::from_millis(80);
    policy.write_stall_timeout = Duration::from_millis(80);
    policy.connection_drain_grace = Duration::from_millis(50);
    let server = tokio::spawn(serve_http(listener, router, policy, controller.token()));

    let io = TokioIo::new(TcpStream::connect(address).await.expect("client connects"));
    let (mut sender, connection) = http2::handshake(TokioExecutor::new(), io)
        .await
        .expect("HTTP/2 handshake succeeds");
    let driver = tokio::spawn(connection);
    let streaming = sender
        .send_request(
            Request::builder()
                .uri("/stream")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("streaming response starts");
    assert_eq!(streaming.status(), StatusCode::OK);

    tokio::time::sleep(Duration::from_millis(180)).await;
    let ready = sender
        .send_request(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("active stream keeps connection admitted");
    assert_eq!(ready.status(), StatusCode::OK);
    drop(ready);
    drop(streaming);

    tokio::time::sleep(Duration::from_millis(180)).await;
    let late = tokio::time::timeout(
        Duration::from_secs(1),
        sender.send_request(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .expect("valid request"),
        ),
    )
    .await
    .expect("idle admission decision should not hang");
    assert!(
        late.is_err(),
        "idle HTTP/2 connection should receive GOAWAY"
    );
    controller.begin_shutdown(ShutdownReason::Sigterm);
    assert!(server.await.expect("listener joins").is_ok());
    driver.abort();
}

#[tokio::test]
async fn slow_reader_receives_complete_response_while_socket_writes_progress() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener address");
    let controller = ShutdownController::new();
    const BODY_BYTES: usize = 8 * 1024 * 1024;
    let router = Router::new().route("/large", get(|| async { vec![b'x'; BODY_BYTES] }));
    let mut policy = test_policy(TrustedProxyHeaders::ignore_all());
    policy.idle_timeout = Duration::from_millis(80);
    policy.connection_drain_grace = Duration::from_millis(50);
    policy.write_stall_timeout = Duration::from_secs(10);
    let server = tokio::spawn(serve_http(listener, router, policy, controller.token()));

    let mut client = TcpStream::connect(address).await.expect("client connects");
    client
        .write_all(b"GET /large HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("request writes");
    let mut received = Vec::new();
    let mut chunk = [0_u8; 16 * 1024];
    timeout(Duration::from_secs(20), async {
        loop {
            let count = client.read(&mut chunk).await.expect("response reads");
            if count == 0 {
                break;
            }
            received.extend_from_slice(&chunk[..count]);
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
    })
    .await
    .expect("slow response completes");
    let header_end = received
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("HTTP headers")
        + 4;
    assert_eq!(received.len() - header_end, BODY_BYTES);
    controller.begin_shutdown(ShutdownReason::Sigterm);
    assert!(server.await.expect("server joins").is_ok());
}

#[tokio::test]
async fn zero_window_http2_response_releases_connection_before_maximum_age() {
    use hyper::client::conn::http2;
    use hyper_util::rt::{TokioExecutor, TokioIo};

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener binds");
    let address = listener.local_addr().expect("listener address");
    let controller = ShutdownController::new();
    // A tiny body can finish producing before Hyper obtains any stream window.
    // The transport must still release the slot before the ordinary idle age.
    let router = Router::new().route("/small", get(|| async { "ok" }));
    let mut policy = test_policy(TrustedProxyHeaders::ignore_all());
    policy.idle_timeout = Duration::from_secs(5);
    policy.write_stall_timeout = Duration::from_millis(80);
    policy.max_connection_age = Duration::from_secs(5);
    let server = tokio::spawn(serve_http(listener, router, policy, controller.token()));

    let io = TokioIo::new(TcpStream::connect(address).await.expect("client connects"));
    let mut builder = http2::Builder::new(TokioExecutor::new());
    builder.initial_stream_window_size(0);
    let (mut sender, connection) = builder.handshake(io).await.expect("h2 handshake");
    let driver = tokio::spawn(connection);
    let response = sender
        .send_request(
            Request::builder()
                .uri("/small")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("response headers");
    assert_eq!(response.status(), StatusCode::OK);
    let _connection_result = timeout(Duration::from_secs(1), driver)
        .await
        .expect("stalled connection must retire before maximum age")
        .expect("client driver joins");
    controller.begin_shutdown(ShutdownReason::Sigterm);
    assert!(server.await.expect("server joins").is_ok());
}

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
        test_policy(TrustedProxyHeaders::ignore_all()),
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

#[tokio::test]
async fn listener_joins_overdue_connection_tasks_before_returning() {
    use std::sync::atomic::{AtomicBool, Ordering};

    struct DropMarker(Arc<AtomicBool>);
    impl Drop for DropMarker {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener binds");
    let address = listener.local_addr().expect("listener address");
    let controller = ShutdownController::new();
    let dropped = Arc::new(AtomicBool::new(false));
    let route_dropped = Arc::clone(&dropped);
    let router = Router::new().route(
        "/stream",
        get(move || {
            let marker = DropMarker(Arc::clone(&route_dropped));
            async move {
                Body::from_stream(stream::once(async move {
                    let _marker = marker;
                    futures_util::future::pending::<Result<Bytes, std::io::Error>>().await
                }))
            }
        }),
    );
    let mut policy = test_policy(TrustedProxyHeaders::ignore_all());
    policy.shutdown_drain_budget = Duration::from_millis(50);
    let server = tokio::spawn(serve_http(listener, router, policy, controller.token()));
    let mut client = TcpStream::connect(address).await.expect("client connects");
    client
        .write_all(b"GET /stream HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("stream request writes");
    let mut headers = [0_u8; 512];
    let received = timeout(Duration::from_secs(1), client.read(&mut headers))
        .await
        .expect("response headers arrive")
        .expect("response headers read");
    assert!(headers[..received].starts_with(b"HTTP/1.1 200"));

    controller.begin_shutdown(ShutdownReason::Sigterm);
    timeout(Duration::from_secs(1), server)
        .await
        .expect("listener drain finishes within its budget")
        .expect("listener task joins")
        .expect("listener returns without a transport error");
    assert!(dropped.load(Ordering::Acquire));
}

#[tokio::test]
async fn trusted_proxy_can_serve_more_than_sixty_four_simultaneous_connections() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener has an address");
    let controller = ShutdownController::new();
    let trusted = TrustedProxyHeaders::trust_configured_proxies(vec![
        TrustedProxyRange::parse("127.0.0.1/32").expect("valid loopback proxy"),
    ])
    .expect("non-empty range");
    let task = tokio::spawn(serve_http(
        listener,
        Router::new().route("/ready", get(|| async { StatusCode::OK })),
        test_policy(trusted),
        controller.token(),
    ));
    let mut clients = Vec::new();
    for _ in 0..65 {
        let mut client = TcpStream::connect(address).await.expect("proxy connects");
        client
            .write_all(b"GET /ready HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .expect("request writes");
        let mut response = [0_u8; 128];
        let length = timeout(Duration::from_secs(2), client.read(&mut response))
            .await
            .expect("response deadline")
            .expect("response read");
        assert!(response[..length].starts_with(b"HTTP/1.1 200"));
        clients.push(client);
    }
    assert_eq!(clients.len(), 65);
    drop(clients);
    controller.begin_shutdown(ShutdownReason::Sigterm);
    assert!(task.await.expect("listener joins").is_ok());
}

#[tokio::test]
async fn response_body_panic_closes_one_connection_without_stopping_listener() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener address");
    let controller = ShutdownController::new();
    let router = Router::new()
        .route(
            "/panic",
            get(|| async {
                Body::from_stream(stream::once(async {
                    panic!("test response body panic");
                    #[allow(unreachable_code)]
                    Ok::<Bytes, std::io::Error>(Bytes::new())
                }))
            }),
        )
        .route("/ready", get(|| async { StatusCode::OK }));
    let task = tokio::spawn(serve_http(
        listener,
        router,
        test_policy(TrustedProxyHeaders::ignore_all()),
        controller.token(),
    ));
    let mut first = TcpStream::connect(address).await.expect("first connection");
    first
        .write_all(b"GET /panic HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("first request");
    let mut discarded = Vec::new();
    let _ = timeout(Duration::from_secs(2), first.read_to_end(&mut discarded))
        .await
        .expect("failed connection closes");

    let mut second = TcpStream::connect(address)
        .await
        .expect("listener still accepts");
    second
        .write_all(b"GET /ready HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("second request");
    let mut response = Vec::new();
    timeout(Duration::from_secs(2), second.read_to_end(&mut response))
        .await
        .expect("second response deadline")
        .expect("second response read");
    assert!(response.starts_with(b"HTTP/1.1 200"));
    controller.begin_shutdown(ShutdownReason::Sigterm);
    assert!(task.await.expect("listener joins").is_ok());
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
        test_policy(TrustedProxyHeaders::ignore_all()),
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
        test_policy(TrustedProxyHeaders::ignore_all()),
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
