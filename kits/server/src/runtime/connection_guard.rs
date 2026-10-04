// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded listener admission and pre-protocol deadlines for HTTP transports.

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::serve::Listener;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, futures::OwnedNotified};
use tokio::time::{Sleep, sleep};

use super::rate_limit::rate_limit_network;

const MAX_LIVE_CONNECTIONS: usize = 2_048;
const MAX_CONNECTIONS_PER_SOURCE: usize = 64;
const FIRST_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONNECTION_AGE: Duration = Duration::from_secs(600);
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(100);

/// The protocol parser's request dispatch is the only reliable indication
/// that an HTTP/1 head or HTTP/2 HEADERS frame was completed.
#[derive(Clone)]
pub(crate) struct FirstRequestTracker(Arc<AtomicBool>);

impl FirstRequestTracker {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub(crate) fn mark_seen(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn was_seen(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[cfg(feature = "tonic-grpc")]
#[derive(Clone)]
pub(crate) struct BoundedTcpConnectInfo {
    tcp: tonic::transport::server::TcpConnectInfo,
    first_request: FirstRequestTracker,
}

#[cfg(feature = "tonic-grpc")]
impl BoundedTcpConnectInfo {
    pub(crate) fn tcp_connect_info(&self) -> tonic::transport::server::TcpConnectInfo {
        self.tcp.clone()
    }

    pub(crate) fn remote_addr(&self) -> Option<SocketAddr> {
        self.tcp.remote_addr()
    }

    pub(crate) fn mark_first_request_seen(&self) {
        self.first_request.mark_seen();
    }
}

pub(super) struct ForceCloseConnections {
    closed: AtomicBool,
    notify: Arc<Notify>,
}

impl ForceCloseConnections {
    #[cfg(feature = "tonic-grpc")]
    pub(super) fn new() -> Self {
        Self {
            closed: AtomicBool::new(false),
            notify: Arc::new(Notify::new()),
        }
    }

    #[cfg(feature = "tonic-grpc")]
    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }
}

/// Listener admission remains active before Hyper or tonic parses a request.
pub(super) struct BoundedTcpListener {
    listener: TcpListener,
    permits: Arc<Semaphore>,
    sources: Arc<Mutex<HashMap<IpAddr, usize>>>,
    force_close: Option<Arc<ForceCloseConnections>>,
}

impl BoundedTcpListener {
    pub(super) fn new(listener: TcpListener) -> Self {
        Self {
            listener,
            permits: Arc::new(Semaphore::new(MAX_LIVE_CONNECTIONS)),
            sources: Arc::new(Mutex::new(HashMap::new())),
            force_close: None,
        }
    }

    #[cfg(feature = "tonic-grpc")]
    pub(super) fn with_force_close(
        listener: TcpListener,
        force_close: Arc<ForceCloseConnections>,
    ) -> Self {
        let mut bounded = Self::new(listener);
        bounded.force_close = Some(force_close);
        bounded
    }
}

impl Listener for BoundedTcpListener {
    type Io = BoundedTcpStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            // Acquire before accept so the process never owns more live
            // connection sockets than its reviewed capacity.
            let permit = match Arc::clone(&self.permits).acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => std::future::pending().await,
            };
            match self.listener.accept().await {
                Ok((stream, peer)) => {
                    let source = rate_limit_network(peer.ip().to_canonical());
                    if let Some(slot) = SourceSlot::acquire(Arc::clone(&self.sources), source) {
                        return (
                            BoundedTcpStream::new(stream, permit, slot, self.force_close.clone()),
                            peer,
                        );
                    }
                    // One peer cannot occupy the whole listener. Closing an
                    // over-cap socket also releases its global permit.
                }
                Err(_) => sleep(ACCEPT_ERROR_BACKOFF).await,
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener.local_addr()
    }
}

struct SourceSlot {
    sources: Arc<Mutex<HashMap<IpAddr, usize>>>,
    source: IpAddr,
}

impl SourceSlot {
    fn acquire(sources: Arc<Mutex<HashMap<IpAddr, usize>>>, source: IpAddr) -> Option<Self> {
        {
            let mut counts = match sources.lock() {
                Ok(counts) => counts,
                Err(poisoned) => poisoned.into_inner(),
            };
            let count = counts.entry(source).or_insert(0);
            if *count >= MAX_CONNECTIONS_PER_SOURCE {
                return None;
            }
            *count = count.checked_add(1)?;
        }
        Some(Self { sources, source })
    }
}

impl Drop for SourceSlot {
    fn drop(&mut self) {
        let mut counts = match self.sources.lock() {
            Ok(counts) => counts,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(count) = counts.get_mut(&self.source) {
            if *count <= 1 {
                counts.remove(&self.source);
            } else {
                *count -= 1;
            }
        }
    }
}

/// IO wrapper owns both permits until the connection task drops the stream.
pub(super) struct BoundedTcpStream {
    io: DeadlineIo<TcpStream>,
    first_request: FirstRequestTracker,
    _permit: OwnedSemaphorePermit,
    _source_slot: SourceSlot,
    force_close: Option<Arc<ForceCloseConnections>>,
    close_notified: Option<Pin<Box<OwnedNotified>>>,
}

impl BoundedTcpStream {
    fn new(
        stream: TcpStream,
        permit: OwnedSemaphorePermit,
        source_slot: SourceSlot,
        force_close: Option<Arc<ForceCloseConnections>>,
    ) -> Self {
        let close_notified = force_close
            .as_ref()
            .map(|state| Box::pin(Arc::clone(&state.notify).notified_owned()));
        let first_request = FirstRequestTracker::new();
        Self {
            io: DeadlineIo::new(
                stream,
                FIRST_REQUEST_TIMEOUT,
                MAX_CONNECTION_AGE,
                first_request.clone(),
            ),
            first_request,
            _permit: permit,
            _source_slot: source_slot,
            force_close,
            close_notified,
        }
    }

    pub(super) fn first_request_tracker(&self) -> FirstRequestTracker {
        self.first_request.clone()
    }

    fn poll_force_close(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        let Some(force_close) = &self.force_close else {
            return Ok(());
        };
        if force_close.closed.load(Ordering::Acquire) {
            return Err(io::ErrorKind::ConnectionAborted.into());
        }
        // Register the connection task's waker on each IO poll. A shutdown
        // signal wakes an otherwise idle HTTP/2 connection after tonic's
        // serving future has been aborted at the drain deadline.
        if self
            .close_notified
            .as_mut()
            .is_some_and(|notified| notified.as_mut().poll(cx).is_ready())
        {
            return Err(io::ErrorKind::ConnectionAborted.into());
        }
        Ok(())
    }
}

impl AsyncRead for BoundedTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.poll_force_close(cx)?;
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl AsyncWrite for BoundedTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_force_close(cx)?;
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_force_close(cx)?;
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_force_close(cx)?;
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
}

#[cfg(feature = "tonic-grpc")]
impl tonic::transport::server::Connected for BoundedTcpStream {
    type ConnectInfo = BoundedTcpConnectInfo;

    fn connect_info(&self) -> Self::ConnectInfo {
        BoundedTcpConnectInfo {
            tcp: self.io.inner.connect_info(),
            first_request: self.first_request.clone(),
        }
    }
}

struct DeadlineIo<T> {
    inner: T,
    first_request_deadline: Pin<Box<Sleep>>,
    max_age_deadline: Pin<Box<Sleep>>,
    first_request: FirstRequestTracker,
}

impl<T> DeadlineIo<T> {
    fn new(
        inner: T,
        first_request: Duration,
        max_age: Duration,
        tracker: FirstRequestTracker,
    ) -> Self {
        Self {
            inner,
            first_request_deadline: Box::pin(sleep(first_request)),
            max_age_deadline: Box::pin(sleep(max_age)),
            first_request: tracker,
        }
    }

    fn timed_out(&mut self, cx: &mut Context<'_>) -> bool {
        self.max_age_deadline.as_mut().poll(cx).is_ready()
            || (!self.first_request.was_seen()
                && self.first_request_deadline.as_mut().poll(cx).is_ready())
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for DeadlineIo<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.timed_out(cx) {
            return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
        }
        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for DeadlineIo<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.timed_out(cx) {
            return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
        }
        Pin::new(&mut this.inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.timed_out(cx) {
            return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
        }
        Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
#[path = "connection_guard_tests.rs"]
mod tests;
