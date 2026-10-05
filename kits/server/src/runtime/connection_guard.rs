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
#[cfg(feature = "tonic-grpc")]
use tokio::time::Sleep;
use tokio::time::sleep;

#[cfg(feature = "tonic-grpc")]
use super::grpc::GrpcTransportTimeouts;
#[cfg(feature = "tonic-grpc")]
use super::grpc_idle::{GrpcConnectionActivity, GrpcRequestActivityGuard};
use super::rate_limit::rate_limit_network;
use crate::config::{ConnectionLimitConfig, TrustedProxyHeaders};

#[path = "connection_guard/deadline_io.rs"]
mod deadline_io;
use deadline_io::DeadlineIo;

const FIRST_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
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
    activity: Option<Arc<GrpcConnectionActivity>>,
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

    pub(crate) fn begin_request(&self) -> Option<Arc<GrpcRequestActivityGuard>> {
        self.activity
            .as_ref()
            .map(GrpcConnectionActivity::begin_request)
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
    limits: ConnectionLimitConfig,
    trusted_proxies: TrustedProxyHeaders,
    force_close: Option<Arc<ForceCloseConnections>>,
    #[cfg(feature = "tonic-grpc")]
    grpc_transport_timeouts: GrpcTransportTimeouts,
}

impl BoundedTcpListener {
    pub(super) fn new(listener: TcpListener, limits: ConnectionLimitConfig) -> Self {
        Self {
            listener,
            permits: Arc::new(Semaphore::new(limits.max_live())),
            sources: Arc::new(Mutex::new(HashMap::new())),
            limits,
            trusted_proxies: TrustedProxyHeaders::ignore_all(),
            force_close: None,
            #[cfg(feature = "tonic-grpc")]
            grpc_transport_timeouts: GrpcTransportTimeouts::default(),
        }
    }

    pub(super) fn with_trusted_proxies(mut self, trusted_proxies: TrustedProxyHeaders) -> Self {
        self.trusted_proxies = trusted_proxies;
        self
    }

    #[cfg(feature = "tonic-grpc")]
    pub(super) fn with_grpc_transport_timeouts(mut self, timeouts: GrpcTransportTimeouts) -> Self {
        self.grpc_transport_timeouts = timeouts;
        self
    }

    #[cfg(feature = "tonic-grpc")]
    pub(super) fn with_force_close(
        listener: TcpListener,
        force_close: Arc<ForceCloseConnections>,
        limits: ConnectionLimitConfig,
    ) -> Self {
        let mut bounded = Self::new(listener, limits);
        bounded.force_close = Some(force_close);
        bounded
    }
}

impl Listener for BoundedTcpListener {
    type Io = BoundedTcpStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match self.listener.accept().await {
                Ok((stream, peer)) => {
                    // Accept and promptly shed over-cap sockets instead of
                    // leaving clients stuck in the kernel's accept backlog.
                    let permit = match Arc::clone(&self.permits).try_acquire_owned() {
                        Ok(permit) => permit,
                        Err(_) => {
                            drop(stream);
                            continue;
                        }
                    };
                    let peer_ip = peer.ip().to_canonical();
                    // A reverse proxy multiplexes many clients on one peer IP.
                    // The global permit still bounds sockets from that peer;
                    // request-level limits use the validated forwarded client.
                    let slot = if source_limit_exempt(peer_ip, &self.trusted_proxies) {
                        Some(None)
                    } else {
                        SourceSlot::acquire(
                            Arc::clone(&self.sources),
                            rate_limit_network(peer_ip),
                            self.limits.max_per_source(),
                        )
                        .map(Some)
                    };
                    if let Some(slot) = slot {
                        return (
                            BoundedTcpStream::new(
                                stream,
                                permit,
                                slot,
                                self.force_close.clone(),
                                #[cfg(feature = "tonic-grpc")]
                                self.grpc_transport_timeouts,
                            ),
                            peer,
                        );
                    }
                    // One peer cannot occupy the whole listener. Closing an
                    // over-cap socket also releases its global permit.
                    drop(stream);
                }
                Err(_) => sleep(ACCEPT_ERROR_BACKOFF).await,
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener.local_addr()
    }
}

fn source_limit_exempt(peer_ip: IpAddr, trusted_proxies: &TrustedProxyHeaders) -> bool {
    peer_ip.is_loopback() || trusted_proxies.trusts_peer(Some(peer_ip))
}

struct SourceSlot {
    sources: Arc<Mutex<HashMap<IpAddr, usize>>>,
    source: IpAddr,
}

impl SourceSlot {
    fn acquire(
        sources: Arc<Mutex<HashMap<IpAddr, usize>>>,
        source: IpAddr,
        max_per_source: usize,
    ) -> Option<Self> {
        {
            let mut counts = match sources.lock() {
                Ok(counts) => counts,
                Err(poisoned) => poisoned.into_inner(),
            };
            let count = counts.entry(source).or_insert(0);
            if *count >= max_per_source {
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
    _source_slot: Option<SourceSlot>,
    force_close: Option<Arc<ForceCloseConnections>>,
    close_notified: Option<Pin<Box<OwnedNotified>>>,
    #[cfg(feature = "tonic-grpc")]
    grpc_activity: Option<Arc<GrpcConnectionActivity>>,
    #[cfg(feature = "tonic-grpc")]
    grpc_hard_deadline: Option<Pin<Box<Sleep>>>,
}

impl BoundedTcpStream {
    fn new(
        stream: TcpStream,
        permit: OwnedSemaphorePermit,
        source_slot: Option<SourceSlot>,
        force_close: Option<Arc<ForceCloseConnections>>,
        #[cfg(feature = "tonic-grpc")] grpc_transport_timeouts: GrpcTransportTimeouts,
    ) -> Self {
        let close_notified = force_close
            .as_ref()
            .map(|state| Box::pin(Arc::clone(&state.notify).notified_owned()));
        let first_request = FirstRequestTracker::new();
        #[cfg(feature = "tonic-grpc")]
        let grpc_activity = force_close.as_ref().map(|_| GrpcConnectionActivity::new());
        #[cfg(feature = "tonic-grpc")]
        let grpc_hard_deadline = grpc_activity
            .as_ref()
            .map(|_| Box::pin(sleep(grpc_transport_timeouts.hard_cap())));
        Self {
            io: DeadlineIo::new(
                stream,
                if force_close.is_some() {
                    #[cfg(feature = "tonic-grpc")]
                    {
                        grpc_transport_timeouts.first_request_timeout()
                    }
                    #[cfg(not(feature = "tonic-grpc"))]
                    {
                        FIRST_REQUEST_TIMEOUT
                    }
                } else {
                    FIRST_REQUEST_TIMEOUT
                },
                first_request.clone(),
            ),
            first_request,
            _permit: permit,
            _source_slot: source_slot,
            force_close,
            close_notified,
            #[cfg(feature = "tonic-grpc")]
            grpc_activity,
            #[cfg(feature = "tonic-grpc")]
            grpc_hard_deadline,
        }
    }

    pub(super) fn first_request_tracker(&self) -> FirstRequestTracker {
        self.first_request.clone()
    }

    #[cfg(feature = "tonic-grpc")]
    pub(super) fn grpc_activity(&self) -> Option<Arc<GrpcConnectionActivity>> {
        self.grpc_activity.clone()
    }

    fn poll_force_close(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        #[cfg(feature = "tonic-grpc")]
        {
            if self
                .grpc_hard_deadline
                .as_mut()
                .is_some_and(|deadline| deadline.as_mut().poll(cx).is_ready())
            {
                return Err(io::ErrorKind::TimedOut.into());
            }
        }
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

    fn mark_transport_closed(&self) {
        #[cfg(feature = "tonic-grpc")]
        if let Some(activity) = &self.grpc_activity {
            activity.mark_transport_closed();
        }
    }

    fn poll_force_close_or_mark(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        let result = self.poll_force_close(cx);
        if result.is_err() {
            self.mark_transport_closed();
        }
        result
    }
}

impl Drop for BoundedTcpStream {
    fn drop(&mut self) {
        self.mark_transport_closed();
    }
}

impl AsyncRead for BoundedTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.poll_force_close_or_mark(cx)?;
        let had_capacity = buf.remaining() != 0;
        let filled_before = buf.filled().len();
        let result = Pin::new(&mut self.io).poll_read(cx, buf);
        if matches!(&result, Poll::Ready(Err(_)))
            || (had_capacity
                && matches!(&result, Poll::Ready(Ok(())))
                && buf.filled().len() == filled_before)
        {
            // A completed nonempty read with no bytes is TCP EOF. Retire the
            // per-connection Tonic future now rather than at its age limit.
            self.mark_transport_closed();
        }
        result
    }
}

impl AsyncWrite for BoundedTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_force_close_or_mark(cx)?;
        let result = Pin::new(&mut self.io).poll_write(cx, buf);
        if matches!(&result, Poll::Ready(Err(_))) {
            self.mark_transport_closed();
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_force_close_or_mark(cx)?;
        let result = Pin::new(&mut self.io).poll_flush(cx);
        if matches!(&result, Poll::Ready(Err(_))) {
            self.mark_transport_closed();
        }
        result
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_force_close_or_mark(cx)?;
        let result = Pin::new(&mut self.io).poll_shutdown(cx);
        if matches!(&result, Poll::Ready(Err(_))) {
            self.mark_transport_closed();
        }
        result
    }
}

#[cfg(feature = "tonic-grpc")]
impl tonic::transport::server::Connected for BoundedTcpStream {
    type ConnectInfo = BoundedTcpConnectInfo;

    fn connect_info(&self) -> Self::ConnectInfo {
        BoundedTcpConnectInfo {
            tcp: self.io.inner.connect_info(),
            first_request: self.first_request.clone(),
            activity: self.grpc_activity.clone(),
        }
    }
}

#[cfg(test)]
#[path = "connection_guard_tests.rs"]
mod tests;
