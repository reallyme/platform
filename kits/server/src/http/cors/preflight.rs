// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use axum::body::Body;
use axum::http::{Method, Request, Response, header};
use pin_project_lite::pin_project;
use tower::{Layer, Service};
use tower_http::cors::{Cors, CorsLayer, ResponseFuture};

/// Lets ordinary OPTIONS requests reach their app route while applying CORS
/// to browser preflights and other methods.
#[derive(Clone)]
pub struct PreflightCorsLayer {
    cors: CorsLayer,
}

impl PreflightCorsLayer {
    pub(super) const fn new(cors: CorsLayer) -> Self {
        Self { cors }
    }
}

impl<S: Clone> Layer<S> for PreflightCorsLayer {
    type Service = PreflightCorsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PreflightCorsService {
            cors: self.cors.layer(inner.clone()),
            inner,
        }
    }
}

/// Per-request CORS dispatch preserving normal OPTIONS routes.
#[derive(Clone)]
pub struct PreflightCorsService<S> {
    cors: Cors<S>,
    inner: S,
}

impl<S> Service<Request<Body>> for PreflightCorsService<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = Infallible> + Clone,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = PreflightCorsFuture<S::Future, ResponseFuture<S::Future>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.inner.poll_ready(cx))?;
        self.cors.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let is_plain_options = request.method() == Method::OPTIONS
            && (!request.headers().contains_key(header::ORIGIN)
                || !request
                    .headers()
                    .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD));
        if is_plain_options {
            PreflightCorsFuture::inner(self.inner.call(request))
        } else {
            PreflightCorsFuture::cors(self.cors.call(request))
        }
    }
}

pin_project! {
    /// Completion of either the app route or tower CORS service.
    pub struct PreflightCorsFuture<F, G> {
        #[pin]
        state: PreflightCorsFutureState<F, G>,
    }
}

pin_project! {
    #[project = PreflightCorsFutureStateProj]
    enum PreflightCorsFutureState<F, G> {
        Inner { #[pin] future: F },
        Cors { #[pin] future: G },
    }
}

impl<F, G> PreflightCorsFuture<F, G> {
    fn inner(future: F) -> Self {
        Self {
            state: PreflightCorsFutureState::Inner { future },
        }
    }

    fn cors(future: G) -> Self {
        Self {
            state: PreflightCorsFutureState::Cors { future },
        }
    }
}

impl<F, G> Future for PreflightCorsFuture<F, G>
where
    F: Future<Output = Result<Response<Body>, Infallible>>,
    G: Future<Output = Result<Response<Body>, Infallible>>,
{
    type Output = Result<Response<Body>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().state.project() {
            PreflightCorsFutureStateProj::Inner { future } => future.poll(cx),
            PreflightCorsFutureStateProj::Cors { future } => future.poll(cx),
        }
    }
}
