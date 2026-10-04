// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Per-connection request activity for safe HTTP keep-alive eviction.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::{Body, Bytes, HttpBody};
use http_body::{Frame, SizeHint};
use tokio::sync::watch;
use tokio::time::Instant;

const MINIMUM_DELIVERY_PROGRESS_BYTES: usize = 16 * 1024;

#[derive(Default)]
struct PendingFrameState {
    frames: HashMap<u64, Instant>,
    written_since_progress: usize,
    written_since_idle_activity: usize,
}

#[derive(Clone, Copy)]
pub(super) struct ConnectionActivitySnapshot {
    pub(super) active_requests: usize,
    pub(super) idle_since: Instant,
    pub(super) has_h2_websocket: bool,
    pub(super) oldest_unconsumed_frame_at: Option<Instant>,
    pub(super) pending_frame_count: usize,
}

#[derive(Clone)]
pub(super) struct ConnectionActivity {
    sender: watch::Sender<ConnectionActivitySnapshot>,
    pending_frames: Arc<Mutex<PendingFrameState>>,
    next_request_id: Arc<AtomicU64>,
}

impl ConnectionActivity {
    pub(super) fn new() -> (Self, watch::Receiver<ConnectionActivitySnapshot>) {
        let (sender, receiver) = watch::channel(ConnectionActivitySnapshot {
            active_requests: 0,
            idle_since: Instant::now(),
            has_h2_websocket: false,
            oldest_unconsumed_frame_at: None,
            pending_frame_count: 0,
        });
        (
            Self {
                sender,
                pending_frames: Arc::new(Mutex::new(PendingFrameState::default())),
                next_request_id: Arc::new(AtomicU64::new(0)),
            },
            receiver,
        )
    }

    pub(super) fn begin_request(&self) -> RequestActivityGuard {
        self.sender.send_modify(|snapshot| {
            // The transport caps simultaneous HTTP/2 streams at 128. Keep
            // the counter non-panicking if that invariant ever changes.
            snapshot.active_requests = snapshot.active_requests.saturating_add(1);
        });
        RequestActivityGuard {
            sender: self.sender.clone(),
            pending_frames: Arc::clone(&self.pending_frames),
            request_id: self.next_request_id.fetch_add(1, Ordering::Relaxed),
        }
    }

    pub(super) fn record_write_progress(&self, written: usize) {
        let mut pending = match self.pending_frames.lock() {
            Ok(pending) => pending,
            Err(poisoned) => poisoned.into_inner(),
        };
        if !pending.frames.is_empty() {
            pending.written_since_progress = pending
                .written_since_progress
                .checked_add(written)
                .unwrap_or(MINIMUM_DELIVERY_PROGRESS_BYTES);
            if pending.written_since_progress >= MINIMUM_DELIVERY_PROGRESS_BYTES {
                let now = Instant::now();
                for last_progress in pending.frames.values_mut() {
                    *last_progress = now;
                }
                pending.written_since_progress = 0;
                self.sender.send_modify(|snapshot| {
                    snapshot.oldest_unconsumed_frame_at = Some(now);
                });
            }
        }
        pending.written_since_idle_activity = pending
            .written_since_idle_activity
            .checked_add(written)
            .unwrap_or(MINIMUM_DELIVERY_PROGRESS_BYTES);
        let enough_response_progress =
            pending.written_since_idle_activity >= MINIMUM_DELIVERY_PROGRESS_BYTES;
        if enough_response_progress {
            pending.written_since_idle_activity = 0;
        }
        if enough_response_progress {
            self.sender.send_modify(|snapshot| {
                // A response can outlive its body producer while Hyper drains
                // queued bytes to a slow reader. Small HTTP/2 keepalive PINGs
                // alone must not keep an otherwise idle connection admitted.
                if snapshot.active_requests == 0 {
                    snapshot.idle_since = Instant::now();
                }
            });
        }
    }

    pub(super) fn mark_h2_websocket(&self) {
        self.sender.send_modify(|snapshot| {
            // Hyper owns the upgraded stream after the response body drops.
            // The connection age still bounds a raw upgraded stream.
            snapshot.has_h2_websocket = true;
        });
    }
}

pub(super) struct RequestActivityGuard {
    sender: watch::Sender<ConnectionActivitySnapshot>,
    pending_frames: Arc<Mutex<PendingFrameState>>,
    request_id: u64,
}

impl RequestActivityGuard {
    fn clear_unconsumed_frame(&self) {
        let mut pending = match self.pending_frames.lock() {
            Ok(pending) => pending,
            Err(poisoned) => poisoned.into_inner(),
        };
        if pending.frames.remove(&self.request_id).is_some() {
            let oldest = pending.frames.values().copied().min();
            if pending.frames.is_empty() {
                pending.written_since_progress = 0;
            }
            self.sender.send_modify(|snapshot| {
                snapshot.oldest_unconsumed_frame_at = oldest;
                snapshot.pending_frame_count = pending.frames.len();
            });
        }
    }

    fn mark_unconsumed_frame(&self) {
        let mut pending = match self.pending_frames.lock() {
            Ok(pending) => pending,
            Err(poisoned) => poisoned.into_inner(),
        };
        let now = Instant::now();
        pending.frames.insert(self.request_id, now);
        let oldest = pending.frames.values().copied().min();
        self.sender.send_modify(|snapshot| {
            snapshot.oldest_unconsumed_frame_at = oldest;
            snapshot.pending_frame_count = pending.frames.len();
        });
    }
}

impl Drop for RequestActivityGuard {
    fn drop(&mut self) {
        self.clear_unconsumed_frame();
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
        if let Some(guard) = &self.request_guard {
            // Hyper can stop polling a body after accepting one DATA frame
            // when the peer's HTTP/2 flow-control window is exhausted.
            guard.clear_unconsumed_frame();
        }
        let frame = self.inner.as_mut().poll_frame(cx);
        if matches!(&frame, Poll::Ready(Some(Ok(frame))) if frame.data_ref().is_some())
            && let Some(guard) = &self.request_guard
        {
            guard.mark_unconsumed_frame();
        }
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
