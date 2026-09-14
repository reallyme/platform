// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use pin_project_lite::pin_project;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, ready};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tower::{Layer, Service};

use crate::config::HttpServerConfig;
use crate::observability::{
    HttpMethodLabel, HttpRejectionReason, MetricRouteTemplateLabel,
    record_http_request_rejected_for_route_template,
};

use super::super::ids::request_id_from_headers;
use super::super::response::JsonErrorResponse;
use super::listener::listener_name_for_request;

/// Layer that bounds concurrent in-flight HTTP requests.
#[derive(Debug, Clone)]
pub struct HttpConcurrencyLimitLayer {
    semaphore: Arc<Semaphore>,
}

/// Creates middleware that bounds concurrent in-flight HTTP requests.
pub fn concurrency_limit_layer(config: &HttpServerConfig) -> HttpConcurrencyLimitLayer {
    HttpConcurrencyLimitLayer {
        semaphore: Arc::new(Semaphore::new(config.concurrency_limit().as_usize())),
    }
}

impl<S> Layer<S> for HttpConcurrencyLimitLayer {
    type Service = HttpConcurrencyLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HttpConcurrencyLimitService {
            inner,
            semaphore: Arc::clone(&self.semaphore),
        }
    }
}

#[derive(Clone)]
pub struct HttpConcurrencyLimitService<S> {
    inner: S,
    semaphore: Arc<Semaphore>,
}

impl<S> Service<Request<Body>> for HttpConcurrencyLimitService<S>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = HttpConcurrencyLimitResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let request_id = request_id_from_headers(request.headers());
        let method = HttpMethodLabel::from_method(request.method());
        let route_template = request.extensions().get::<MatchedPath>().cloned();
        let listener_name = listener_name_for_request(&request);

        match Arc::clone(&self.semaphore).try_acquire_owned() {
            Ok(permit) => {
                let response_future = self.inner.call(request);
                HttpConcurrencyLimitResponseFuture::inner(response_future, permit)
            }
            Err(_) => {
                let route_template =
                    MetricRouteTemplateLabel::from_matched_path(route_template.as_ref());
                record_http_request_rejected_for_route_template(
                    listener_name,
                    method,
                    &route_template,
                    HttpRejectionReason::ConcurrencyLimit,
                );

                HttpConcurrencyLimitResponseFuture::ready(
                    JsonErrorResponse::service_unavailable()
                        .with_optional_request_id(request_id)
                        .into_response(),
                )
            }
        }
    }
}

pin_project! {
    /// Response future for [`HttpConcurrencyLimitService`].
    pub struct HttpConcurrencyLimitResponseFuture<F> {
        #[pin]
        state: HttpConcurrencyLimitResponseFutureState<F>,
    }
}

pin_project! {
    #[project = HttpConcurrencyLimitResponseFutureStateProj]
    enum HttpConcurrencyLimitResponseFutureState<F> {
        Inner {
            #[pin]
            inner: F,
            permit: Option<OwnedSemaphorePermit>,
        },
        Ready {
            response: Option<Response>,
        },
    }
}

impl<F> HttpConcurrencyLimitResponseFuture<F> {
    fn inner(inner: F, permit: OwnedSemaphorePermit) -> Self {
        Self {
            state: HttpConcurrencyLimitResponseFutureState::Inner {
                inner,
                permit: Some(permit),
            },
        }
    }

    fn ready(response: Response) -> Self {
        Self {
            state: HttpConcurrencyLimitResponseFutureState::Ready {
                response: Some(response),
            },
        }
    }
}

impl<F> Future for HttpConcurrencyLimitResponseFuture<F>
where
    F: Future<Output = Result<Response, Infallible>>,
{
    type Output = Result<Response, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().state.project() {
            HttpConcurrencyLimitResponseFutureStateProj::Inner { inner, permit } => {
                let response = ready!(inner.poll(cx))?;
                let _permit = permit.take();
                Poll::Ready(Ok(response))
            }
            HttpConcurrencyLimitResponseFutureStateProj::Ready { response } => {
                let response = match response.take() {
                    Some(response) => response,
                    None => JsonErrorResponse::internal_server_error().into_response(),
                };

                Poll::Ready(Ok(response))
            }
        }
    }
}
