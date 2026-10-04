// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(feature = "tonic-grpc")]
use axum::serve::Listener;
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(feature = "tonic-grpc")]
use super::{BoundedTcpListener, ForceCloseConnections};
use super::{DeadlineIo, FirstRequestTracker, MAX_CONNECTIONS_PER_SOURCE, SourceSlot};

#[cfg(feature = "tonic-grpc")]
#[tokio::test]
async fn force_close_interrupts_idle_connection_io() {
    let socket = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind local fixture");
    let address = socket.local_addr().expect("local address");
    let force_close = Arc::new(ForceCloseConnections::new());
    let mut listener = BoundedTcpListener::with_force_close(socket, Arc::clone(&force_close));
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

#[test]
fn one_source_cannot_occupy_every_connection_slot() {
    let sources = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let source = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mut slots = Vec::new();
    for _ in 0..MAX_CONNECTIONS_PER_SOURCE {
        slots.push(SourceSlot::acquire(Arc::clone(&sources), source).expect("slot"));
    }
    assert!(SourceSlot::acquire(Arc::clone(&sources), source).is_none());
    drop(slots.pop());
    assert!(SourceSlot::acquire(Arc::clone(&sources), source).is_some());
    drop(slots);
}

#[test]
fn rotating_ipv6_interface_ids_share_one_connection_source() {
    let sources = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let first = "2001:db8:1:2::1".parse::<std::net::IpAddr>().expect("IPv6");
    let mut slots = Vec::new();
    for offset in 0..MAX_CONNECTIONS_PER_SOURCE {
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
            )
            .expect("slot within source cap"),
        );
    }
    assert!(SourceSlot::acquire(Arc::clone(&sources), super::rate_limit_network(first)).is_none());
    drop(slots);
}

#[tokio::test]
async fn zero_byte_connection_expires_before_protocol_detection() {
    let (_writer, reader) = tokio::io::duplex(64);
    let mut io = DeadlineIo::new(
        reader,
        Duration::from_millis(20),
        Duration::from_secs(1),
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
        Duration::from_secs(1),
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
        Duration::from_secs(1),
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
    let mut io = DeadlineIo::new(
        reader,
        Duration::from_millis(20),
        Duration::from_secs(1),
        tracker.clone(),
    );
    tracker.mark_seen();
    tokio::time::sleep(Duration::from_millis(30)).await;
    writer.write_all(b"next").await.expect("fixture write");
    let mut buffer = [0u8; 32];
    assert_eq!(io.read(&mut buffer).await.expect("post-request read"), 4);
}
