// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Per-connection request activity for safe HTTP keep-alive eviction.

use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::{Body, Bytes, HttpBody};
use http_body::{Frame, SizeHint};
use tokio::sync::watch;
use tokio::time::Instant;

#[derive(Clone, Copy)]
pub(super) struct ConnectionActivitySnapshot {
    pub(super) active_requests: usize,
    pub(super) idle_since: Instant,
}

#[derive(Clone)]
pub(super) struct ConnectionActivity {
    sender: watch::Sender<ConnectionActivitySnapshot>,
}

impl ConnectionActivity {
    pub(super) fn new() -> (Self, watch::Receiver<ConnectionActivitySnapshot>) {
        let (sender, receiver) = watch::channel(ConnectionActivitySnapshot {
            active_requests: 0,
            idle_since: Instant::now(),
        });
        (Self { sender }, receiver)
    }

    pub(super) fn begin_request(&self) -> RequestActivityGuard {
        self.sender.send_modify(|snapshot| {
            // The transport caps simultaneous HTTP/2 streams at 128. Keep
            // the counter non-panicking if that invariant ever changes.
            snapshot.active_requests = snapshot.active_requests.saturating_add(1);
        });
        RequestActivityGuard {
            sender: self.sender.clone(),
        }
    }
}

pub(super) struct RequestActivityGuard {
    sender: watch::Sender<ConnectionActivitySnapshot>,
}

impl Drop for RequestActivityGuard {
    fn drop(&mut self) {
        self.sender.send_modify(|snapshot| {
            snapshot.active_requests = snapshot.active_requests.saturating_sub(1);
            if snapshot.active_requests == 0 {
                snapshot.idle_since = Instant::now();
            }
        });
    }
}

/// A response remains active until its final frame or connection teardown.
/// This prevents a long-running stream from being classified as idle.
pub(super) struct TrackedResponseBody {
    inner: Pin<Box<Body>>,
    request_guard: Option<RequestActivityGuard>,
}

impl TrackedResponseBody {
    pub(super) fn new(inner: Body, request_guard: RequestActivityGuard) -> Self {
        Self {
            inner: Box::pin(inner),
            request_guard: Some(request_guard),
        }
    }
}

impl HttpBody for TrackedResponseBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let frame = self.inner.as_mut().poll_frame(cx);
        if matches!(frame, Poll::Ready(None)) {
            self.request_guard = None;
        }
        frame
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}
