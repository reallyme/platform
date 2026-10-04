// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Request-aware idle tracking for native gRPC connections.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, LockResult, Mutex, MutexGuard};
use std::task::{Context, Poll, ready};

use axum::body::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use pin_project_lite::pin_project;
use tokio::sync::watch;
use tokio::time::Instant;
use tonic::body::Body as TonicBody;
use tonic::codegen::http::{Request, Response};
use tower::{Layer, Service};

use super::connection_guard::BoundedTcpConnectInfo;

struct ActivityState {
    active_requests: usize,
    idle_since: Instant,
}

/// Tracks complete request streams rather than HTTP/2 ping or socket activity.
pub(crate) struct GrpcConnectionActivity {
    state: Mutex<ActivityState>,
    idle_sender: watch::Sender<Option<Instant>>,
}

impl GrpcConnectionActivity {
    pub(crate) fn new() -> Arc<Self> {
        let now = Instant::now();
        let (idle_sender, _idle_receiver) = watch::channel(Some(now));
        Arc::new(Self {
            state: Mutex::new(ActivityState {
                active_requests: 0,
                idle_since: now,
            }),
            idle_sender,
        })
    }

    pub(crate) fn begin_request(self: &Arc<Self>) -> Arc<GrpcRequestActivityGuard> {
        {
            let mut state = recover_lock(self.state.lock());
            state.active_requests = state.active_requests.saturating_add(1);
            self.idle_sender.send_replace(None);
        }
        Arc::new(GrpcRequestActivityGuard {
            activity: Arc::clone(self),
        })
    }

    pub(crate) fn subscribe_idle(&self) -> watch::Receiver<Option<Instant>> {
        self.idle_sender.subscribe()
    }
}

fn recover_lock(
    result: LockResult<MutexGuard<'_, ActivityState>>,
) -> MutexGuard<'_, ActivityState> {
    match result {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(crate) struct GrpcRequestActivityGuard {
    activity: Arc<GrpcConnectionActivity>,
}

impl Drop for GrpcRequestActivityGuard {
    fn drop(&mut self) {
        {
            let mut state = recover_lock(self.activity.state.lock());
            state.active_requests = state.active_requests.saturating_sub(1);
            if state.active_requests == 0 {
                state.idle_since = Instant::now();
                self.activity
                    .idle_sender
                    .send_replace(Some(state.idle_since));
            }
        }
    }
}

/// Keeps a gRPC request active while either input or output is still owned.
struct TrackedGrpcBody {
    inner: Pin<Box<TonicBody>>,
    guard: Option<Arc<GrpcRequestActivityGuard>>,
}

impl TrackedGrpcBody {
    fn new(inner: TonicBody, guard: Arc<GrpcRequestActivityGuard>) -> Self {
        Self {
            inner: Box::pin(inner),
            guard: Some(guard),
        }
    }
}

impl HttpBody for TrackedGrpcBody {
    type Data = Bytes;
    type Error = tonic::Status;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let frame = self.inner.as_mut().poll_frame(cx);
        if matches!(frame, Poll::Ready(None)) {
            self.guard = None;
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

#[derive(Clone, Copy)]
pub(crate) struct GrpcActivityLayer;

impl<S> Layer<S> for GrpcActivityLayer {
    type Service = GrpcActivityService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcActivityService { inner }
    }
}

#[derive(Clone)]
pub(crate) struct GrpcActivityService<S> {
    inner: S,
}

impl<S> Service<Request<TonicBody>> for GrpcActivityService<S>
where
    S: Service<Request<TonicBody>, Response = Response<TonicBody>>,
{
    type Response = Response<TonicBody>;
    type Error = S::Error;
    type Future = GrpcActivityResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<TonicBody>) -> Self::Future {
        let guard = request
            .extensions()
            .get::<BoundedTcpConnectInfo>()
            .and_then(BoundedTcpConnectInfo::begin_request);
        if let Some(activity) = &guard {
            request = request
                .map(|body| TonicBody::new(TrackedGrpcBody::new(body, Arc::clone(activity))));
        }
        GrpcActivityResponseFuture {
            inner: self.inner.call(request),
            guard,
        }
    }
}

pin_project! {
    pub(crate) struct GrpcActivityResponseFuture<F> {
        #[pin]
        inner: F,
        guard: Option<Arc<GrpcRequestActivityGuard>>,
    }
}

impl<F, E> Future for GrpcActivityResponseFuture<F>
where
    F: Future<Output = Result<Response<TonicBody>, E>>,
{
    type Output = Result<Response<TonicBody>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let response = ready!(this.inner.poll(cx))?;
        let response = match this.guard.take() {
            Some(guard) => response.map(|body| TonicBody::new(TrackedGrpcBody::new(body, guard))),
            None => response,
        };
        Poll::Ready(Ok(response))
    }
}
