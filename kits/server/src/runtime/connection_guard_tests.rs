// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(feature = "tonic-grpc")]
use axum::serve::Listener;
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[cfg(feature = "tonic-grpc")]
use tonic::transport::server::Connected;
#[cfg(feature = "tonic-grpc")]
use tower::{Layer, Service};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(feature = "tonic-grpc")]
use super::{BoundedTcpListener, ForceCloseConnections};
use super::{DeadlineIo, FirstRequestTracker, SourceSlot};
#[cfg(feature = "tonic-grpc")]
use crate::config::ConnectionLimitConfig;
use crate::config::{TrustedProxyHeaders, TrustedProxyRange};

const TEST_SOURCE_LIMIT: usize = 64;

#[cfg(feature = "tonic-grpc")]
impl BoundedTcpListener {
    fn with_grpc_idle_timeout(mut self, timeout: Duration) -> Self {
        self.grpc_idle_timeout = timeout;
        self
    }
}

#[test]
fn configured_non_loopback_proxy_is_exempt_from_source_cap() {
    let trusted = TrustedProxyHeaders::trust_configured_proxies(vec![
        TrustedProxyRange::parse("10.42.0.0/16").expect("valid fixture range"),
    ])
    .expect("non-empty fixture range");
    let proxy = "10.42.1.9".parse().expect("valid proxy address");
    let direct = "192.0.2.9".parse().expect("valid direct address");

    assert!(super::source_limit_exempt(proxy, &trusted));
    assert!(!super::source_limit_exempt(direct, &trusted));
    assert!(super::source_limit_exempt(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        &TrustedProxyHeaders::ignore_all()
    ));
}

#[cfg(feature = "tonic-grpc")]
#[tokio::test]
async fn force_close_interrupts_idle_connection_io() {
    let socket = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind local fixture");
    let address = socket.local_addr().expect("local address");
    let force_close = Arc::new(ForceCloseConnections::new());
    let mut listener = BoundedTcpListener::with_force_close(
        socket,
        Arc::clone(&force_close),
        ConnectionLimitConfig::secure_defaults(),
    );
    let client = tokio::net::TcpStream::connect(address)
        .await
        .expect("connect local fixture");
    let (mut server, _) = listener.accept().await;
    let read = tokio::spawn(async move {
        let mut buffer = [0u8; 1];
        server.read(&mut buffer).await
    });
    tokio::task::yield_now().await;
    force_close.close();
    let result = tokio::time::timeout(Duration::from_secs(1), read)
        .await
        .expect("connection task should wake")
        .expect("connection task should complete");
    assert_eq!(
        result.expect_err("force close must interrupt IO").kind(),
        io::ErrorKind::ConnectionAborted
    );
    drop(client);
}

#[cfg(feature = "tonic-grpc")]
#[tokio::test]
async fn global_capacity_sheds_new_sockets_without_blocking_accept() {
    let socket = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind local fixture");
    let address = socket.local_addr().expect("local address");
    let limits = ConnectionLimitConfig::new(1, 1).expect("valid fixture limits");
    let mut listener = BoundedTcpListener::new(socket, limits);

    let first_client = tokio::net::TcpStream::connect(address)
        .await
        .expect("first client connects");
    let (first_server, _) = listener.accept().await;
    let accept_next = tokio::spawn(async move { listener.accept().await });
    let mut second_client = tokio::net::TcpStream::connect(address)
        .await
        .expect("second client connects");
    let mut byte = [0_u8; 1];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), second_client.read(&mut byte))
            .await
            .expect("over-cap socket should be shed promptly")
            .expect("socket read"),
        0
    );

    drop(first_client);
    drop(first_server);
    let third_client = tokio::net::TcpStream::connect(address)
        .await
        .expect("third client connects");
    let (_third_server, _) = tokio::time::timeout(Duration::from_secs(1), accept_next)
        .await
        .expect("admission resumes")
        .expect("listener task joins");
    drop(third_client);
}

#[cfg(feature = "tonic-grpc")]
#[tokio::test]
async fn grpc_idle_timeout_waits_for_active_streams_then_retires_the_connection() {
    let socket = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind local fixture");
    let address = socket.local_addr().expect("local address");
    let mut listener = BoundedTcpListener::with_force_close(
        socket,
        Arc::new(ForceCloseConnections::new()),
        ConnectionLimitConfig::new(1, 1).expect("valid limits"),
    )
    .with_grpc_idle_timeout(Duration::from_millis(50));
    let _client = tokio::net::TcpStream::connect(address)
        .await
        .expect("connect local fixture");
    let (mut server, _) = listener.accept().await;
    let connection = server.connect_info();
    connection.mark_first_request_seen();
    let mut request = tonic::codegen::http::Request::new(tonic::body::Body::empty());
    request.extensions_mut().insert(connection);
    let mut service = crate::runtime::grpc_idle::GrpcActivityLayer.layer(tower::service_fn(
        |_request: tonic::codegen::http::Request<tonic::body::Body>| async {
            Ok::<_, std::convert::Infallible>(tonic::codegen::http::Response::new(
                tonic::body::Body::new(PendingGrpcBody),
            ))
        },
    ));
    let response = service.call(request).await.expect("infallible service");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut byte = [0_u8; 1];
    assert!(
        tokio::time::timeout(Duration::from_millis(20), server.read(&mut byte))
            .await
            .is_err(),
        "an active stream must survive the idle interval"
    );
    drop(response);
    let error = tokio::time::timeout(Duration::from_secs(1), server.read(&mut byte))
        .await
        .expect("idle socket should wake")
        .expect_err("idle socket must close");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[cfg(feature = "tonic-grpc")]
struct PendingGrpcBody;

#[cfg(feature = "tonic-grpc")]
impl http_body::Body for PendingGrpcBody {
    type Data = axum::body::Bytes;
    type Error = io::Error;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        std::task::Poll::Pending
    }

    fn size_hint(&self) -> http_body::SizeHint {
        http_body::SizeHint::default()
    }
}

#[test]
fn one_source_cannot_occupy_every_connection_slot() {
    let sources = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let source = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mut slots = Vec::new();
    for _ in 0..TEST_SOURCE_LIMIT {
        slots.push(
            SourceSlot::acquire(Arc::clone(&sources), source, TEST_SOURCE_LIMIT).expect("slot"),
        );
    }
    assert!(SourceSlot::acquire(Arc::clone(&sources), source, TEST_SOURCE_LIMIT).is_none());
    drop(slots.pop());
    assert!(SourceSlot::acquire(Arc::clone(&sources), source, TEST_SOURCE_LIMIT).is_some());
    drop(slots);
}

#[test]
fn rotating_ipv6_interface_ids_share_one_connection_source() {
    let sources = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let first = "2001:db8:1:2::1".parse::<std::net::IpAddr>().expect("IPv6");
    let mut slots = Vec::new();
    for offset in 0..TEST_SOURCE_LIMIT {
        let address = std::net::Ipv6Addr::from(
            u128::from(
                "2001:db8:1:2::"
                    .parse::<std::net::Ipv6Addr>()
                    .expect("IPv6"),
            ) + u128::try_from(offset).expect("bounded test offset"),
        );
        slots.push(
            SourceSlot::acquire(
                Arc::clone(&sources),
                super::rate_limit_network(IpAddr::V6(address)),
                TEST_SOURCE_LIMIT,
            )
            .expect("slot within source cap"),
        );
    }
    assert!(
        SourceSlot::acquire(
            Arc::clone(&sources),
            super::rate_limit_network(first),
            TEST_SOURCE_LIMIT
        )
        .is_none()
    );
    drop(slots);
}

#[tokio::test]
async fn zero_byte_connection_expires_before_protocol_detection() {
    let (_writer, reader) = tokio::io::duplex(64);
    let mut io = DeadlineIo::new(
        reader,
        Duration::from_millis(20),
        FirstRequestTracker::new(),
    );
    let mut buffer = [0u8; 32];
    let error = io
        .read(&mut buffer)
        .await
        .expect_err("first request deadline");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[tokio::test]
async fn partial_http2_preface_expires() {
    let (mut writer, reader) = tokio::io::duplex(64);
    let mut io = DeadlineIo::new(
        reader,
        Duration::from_millis(20),
        FirstRequestTracker::new(),
    );
    writer
        .write_all(b"PRI * HTTP/2.0\r\n\r\n")
        .await
        .expect("fixture write");
    let mut buffer = [0u8; 32];
    assert!(io.read(&mut buffer).await.expect("partial prefix") > 0);
    let error = io
        .read(&mut buffer)
        .await
        .expect_err("remaining preface deadline");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[tokio::test]
async fn complete_http2_preface_does_not_release_first_request_deadline() {
    let (mut writer, reader) = tokio::io::duplex(64);
    let mut io = DeadlineIo::new(
        reader,
        Duration::from_millis(20),
        FirstRequestTracker::new(),
    );
    writer
        .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
        .await
        .expect("fixture write");
    let mut buffer = [0u8; 32];
    assert_eq!(io.read(&mut buffer).await.expect("complete preface"), 24);
    let error = io
        .read(&mut buffer)
        .await
        .expect_err("first request deadline");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[tokio::test]
async fn dispatched_request_releases_first_request_deadline() {
    let (mut writer, reader) = tokio::io::duplex(64);
    let tracker = FirstRequestTracker::new();
    let mut io = DeadlineIo::new(reader, Duration::from_millis(20), tracker.clone());
    tracker.mark_seen();
    tokio::time::sleep(Duration::from_millis(30)).await;
    writer.write_all(b"next").await.expect("fixture write");
    let mut buffer = [0u8; 32];
    assert_eq!(io.read(&mut buffer).await.expect("post-request read"), 4);
}
