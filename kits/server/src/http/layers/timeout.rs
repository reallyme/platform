// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::Body;
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use pin_project_lite::pin_project;
use tokio::time::{Sleep, sleep};
use tower::{Layer, Service};

use crate::config::HttpServerConfig;
use crate::transport::RequestId;

use super::super::ids::request_id_from_headers;
use super::super::response::JsonErrorResponse;

/// Layer that enforces a stable request timeout.
#[derive(Debug, Clone, Copy)]
pub struct HttpTimeoutLayer {
    request_timeout: Duration,
}

/// Creates request-timeout middleware with a stable JSON timeout response.
pub fn timeout_layer(config: &HttpServerConfig) -> HttpTimeoutLayer {
    HttpTimeoutLayer {
        request_timeout: config.timeout().request_timeout().as_duration(),
    }
}

impl<S> Layer<S> for HttpTimeoutLayer {
    type Service = HttpTimeoutService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HttpTimeoutService {
            inner,
            request_timeout: self.request_timeout,
        }
    }
}

#[derive(Clone)]
pub struct HttpTimeoutService<S> {
    inner: S,
    request_timeout: Duration,
}

impl<S> Service<Request<Body>> for HttpTimeoutService<S>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = HttpTimeoutResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let request_id = request_id_from_headers(request.headers());
        let request_timeout = self.request_timeout;
        let response_future = self.inner.call(request);

        HttpTimeoutResponseFuture::new(response_future, request_timeout, request_id)
    }
}

pin_project! {
    /// Response future for [`HttpTimeoutService`].
    pub struct HttpTimeoutResponseFuture<F> {
        #[pin]
        inner: F,
        #[pin]
        sleep: Option<Sleep>,
        request_timeout: Duration,
        request_id: Option<RequestId>,
    }
}

impl<F> HttpTimeoutResponseFuture<F> {
    fn new(inner: F, request_timeout: Duration, request_id: Option<RequestId>) -> Self {
        Self {
            inner,
            sleep: None,
            request_timeout,
            request_id,
        }
    }
}

impl<F> Future for HttpTimeoutResponseFuture<F>
where
    F: Future<Output = Result<Response, Infallible>>,
{
    type Output = Result<Response, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if let Poll::Ready(response) = this.inner.poll(cx) {
            return Poll::Ready(response);
        }

        let sleep_poll = match this.sleep.as_mut().as_pin_mut() {
            Some(sleep) => sleep.poll(cx),
            None => {
                this.sleep.set(Some(sleep(*this.request_timeout)));
                match this.sleep.as_mut().as_pin_mut() {
                    Some(sleep) => sleep.poll(cx),
                    None => Poll::Pending,
                }
            }
        };

        if sleep_poll.is_ready() {
            return Poll::Ready(Ok(JsonErrorResponse::request_timeout()
                .with_optional_request_id(*this.request_id)
                .into_response()));
        }

        Poll::Pending
    }
}

#[cfg(test)]
#[path = "timeout_tests.rs"]
mod tests;
