// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use axum::body::Body;
use axum::http::{HeaderValue, Request};
use axum::response::{IntoResponse, Response};
use pin_project_lite::pin_project;
use tower::{Layer, Service};

use crate::observability::{
    HttpMethodLabel, HttpRejectionReason, MetricRouteTemplateLabel,
    record_http_request_rejected_for_route_template,
};
use crate::transport::RequestId;

use super::super::ids::{X_REQUEST_ID, request_id_from_headers};
use super::super::response::JsonErrorResponse;
use super::listener::listener_name_for_request;

/// Layer that validates or replaces the request ID.
#[derive(Debug, Clone, Copy, Default)]
pub struct RequestIdLayer;

/// Creates middleware that validates or replaces the request ID.
///
/// Policy:
/// - if a valid request ID header is present, preserve it
/// - if the header is missing, generate a new request ID
/// - if the header is malformed, replace it with a newly generated request ID
pub fn request_id_layer() -> RequestIdLayer {
    RequestIdLayer
}

impl<S> Layer<S> for RequestIdLayer {
    type Service = RequestIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestIdService { inner }
    }
}

#[derive(Clone)]
pub struct RequestIdService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for RequestIdService<S>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = RequestIdResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        if let Some(header_value) = request.headers().get(X_REQUEST_ID)
            && RequestId::try_from(header_value).is_err()
        {
            let route_template = MetricRouteTemplateLabel::unknown();
            record_http_request_rejected_for_route_template(
                listener_name_for_request(&request),
                HttpMethodLabel::from_method(request.method()),
                &route_template,
                HttpRejectionReason::MalformedRequestId,
            );
        }

        let request_id =
            request_id_from_headers(request.headers()).unwrap_or_else(RequestId::generate);
        let header_value = match request_id.to_header_value() {
            Ok(header_value) => header_value,
            Err(_) => {
                return RequestIdResponseFuture::ready(
                    JsonErrorResponse::internal_server_error().into_response(),
                );
            }
        };
        request.extensions_mut().insert(request_id);
        request
            .headers_mut()
            .insert(X_REQUEST_ID, header_value.clone());
        let response_future = self.inner.call(request);

        RequestIdResponseFuture::new(response_future, request_id, header_value)
    }
}

pin_project! {
    /// Response future for [`RequestIdService`].
    pub struct RequestIdResponseFuture<F> {
        #[pin]
        state: RequestIdResponseFutureState<F>,
    }
}

pin_project! {
    #[project = RequestIdResponseFutureStateProj]
    enum RequestIdResponseFutureState<F> {
        Inner {
            #[pin]
            inner: F,
            request_id: RequestId,
            header_value: Option<HeaderValue>,
        },
        Ready {
            response: Option<Response>,
        },
    }
}

impl<F> RequestIdResponseFuture<F> {
    fn new(inner: F, request_id: RequestId, header_value: HeaderValue) -> Self {
        Self {
            state: RequestIdResponseFutureState::Inner {
                inner,
                request_id,
                header_value: Some(header_value),
            },
        }
    }

    fn ready(response: Response) -> Self {
        Self {
            state: RequestIdResponseFutureState::Ready {
                response: Some(response),
            },
        }
    }
}

impl<F> Future for RequestIdResponseFuture<F>
where
    F: Future<Output = Result<Response, Infallible>>,
{
    type Output = Result<Response, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().state.project() {
            RequestIdResponseFutureStateProj::Inner {
                inner,
                request_id,
                header_value,
            } => {
                let mut response = ready!(inner.poll(cx))?;

                response.extensions_mut().insert(*request_id);
                if !response.headers().contains_key(X_REQUEST_ID)
                    && let Some(header_value) = header_value.take()
                {
                    response.headers_mut().insert(X_REQUEST_ID, header_value);
                }

                Poll::Ready(Ok(response))
            }
            RequestIdResponseFutureStateProj::Ready { response } => {
                let response = match response.take() {
                    Some(response) => response,
                    None => JsonErrorResponse::internal_server_error().into_response(),
                };

                Poll::Ready(Ok(response))
            }
        }
    }
}
