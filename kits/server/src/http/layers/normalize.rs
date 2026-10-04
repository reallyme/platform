// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use pin_project_lite::pin_project;
use tower::{Layer, Service};

use crate::http::HttpListenerName;
use crate::observability::{
    HttpMethodLabel, HttpRejectionReason, MetricRouteTemplateLabel,
    record_http_request_rejected_for_route_template,
};
use crate::transport::RequestId;

use super::super::content_type::header_value_is_json_content_type;
use super::super::error::{ErrorCode, PublicHttpError};
use super::super::ids::request_id_from_headers;
use super::super::response::JsonErrorResponse;
use super::listener::listener_name_for_request;

/// Layer that normalizes transport-generated HTTP error responses into the
/// stable JSON public error envelope.
#[derive(Debug, Clone, Copy, Default)]
pub struct NormalizeHttpErrorResponsesLayer;

/// Creates middleware that normalizes transport-generated HTTP errors into the
/// stable JSON envelope where `server-kit` has a deliberate public mapping.
pub fn normalize_http_error_responses_layer() -> NormalizeHttpErrorResponsesLayer {
    NormalizeHttpErrorResponsesLayer
}

impl<S> Layer<S> for NormalizeHttpErrorResponsesLayer {
    type Service = NormalizeHttpErrorResponsesService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        NormalizeHttpErrorResponsesService { inner }
    }
}

#[derive(Clone)]
pub struct NormalizeHttpErrorResponsesService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for NormalizeHttpErrorResponsesService<S>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = NormalizeHttpErrorResponsesFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let request_id = request_id_from_headers(request.headers());
        let method = HttpMethodLabel::from_method(request.method());
        let route_template =
            MetricRouteTemplateLabel::from_matched_path(request.extensions().get::<MatchedPath>());
        let listener_name = listener_name_for_request(&request).clone();
        let unsupported_websocket_version = request
            .headers()
            .get(header::UPGRADE)
            .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"websocket"))
            && request
                .headers()
                .get(header::SEC_WEBSOCKET_VERSION)
                .is_some_and(|value| value != "13");
        let response_future = self.inner.call(request);

        NormalizeHttpErrorResponsesFuture::new(
            response_future,
            request_id,
            method,
            route_template,
            listener_name,
            unsupported_websocket_version,
        )
    }
}

pin_project! {
    /// Response future for [`NormalizeHttpErrorResponsesService`].
    pub struct NormalizeHttpErrorResponsesFuture<F> {
        #[pin]
        inner: F,
        request_id: Option<RequestId>,
        method: HttpMethodLabel,
        route_template: MetricRouteTemplateLabel,
        listener_name: HttpListenerName,
        unsupported_websocket_version: bool,
    }
}

impl<F> NormalizeHttpErrorResponsesFuture<F> {
    fn new(
        inner: F,
        request_id: Option<RequestId>,
        method: HttpMethodLabel,
        route_template: MetricRouteTemplateLabel,
        listener_name: HttpListenerName,
        unsupported_websocket_version: bool,
    ) -> Self {
        Self {
            inner,
            request_id,
            method,
            route_template,
            listener_name,
            unsupported_websocket_version,
        }
    }
}

impl<F> Future for NormalizeHttpErrorResponsesFuture<F>
where
    F: Future<Output = Result<Response, Infallible>>,
{
    type Output = Result<Response, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let response = ready!(this.inner.poll(cx))?;
        record_audit_rejection_metric(
            this.listener_name,
            *this.method,
            this.route_template,
            response.status(),
        );

        let mut response = match public_error_for_transport_response(&response) {
            Some(public_error_response) => {
                let mut normalized_response = public_error_response
                    .with_optional_request_id(*this.request_id)
                    .into_response();
                preserve_safe_protocol_headers(&response, &mut normalized_response);
                normalized_response
            }
            None => response,
        };

        if *this.unsupported_websocket_version && response.status() == StatusCode::BAD_REQUEST {
            // Axum rejects unsupported versions before the application handler.
            // RFC 6455 requires the supported version in that rejection.
            response.headers_mut().insert(
                header::SEC_WEBSOCKET_VERSION,
                HeaderValue::from_static("13"),
            );
        }

        Poll::Ready(Ok(response))
    }
}

fn record_audit_rejection_metric(
    listener_name: &HttpListenerName,
    method: HttpMethodLabel,
    route_template: &MetricRouteTemplateLabel,
    status: StatusCode,
) {
    let reason = match status {
        StatusCode::UNAUTHORIZED => HttpRejectionReason::AuthRejected,
        StatusCode::UNSUPPORTED_MEDIA_TYPE => HttpRejectionReason::InvalidContentType,
        _ => return,
    };

    record_http_request_rejected_for_route_template(listener_name, method, route_template, reason);
}

fn public_error_for_transport_response(response: &Response) -> Option<JsonErrorResponse> {
    // Connect advertises supported request encodings in Accept-Post on a
    // bodyless 415. Replacing that response with a platform envelope would
    // discard the protocol signal and change the error seen by Connect clients.
    if response.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE
        && response.headers().contains_key("accept-post")
    {
        return None;
    }
    let error_code = match response.status() {
        StatusCode::BAD_REQUEST => ErrorCode::BadRequest,
        StatusCode::UNAUTHORIZED => ErrorCode::Unauthorized,
        StatusCode::FORBIDDEN => ErrorCode::Forbidden,
        StatusCode::NOT_FOUND => ErrorCode::NotFound,
        StatusCode::METHOD_NOT_ALLOWED => ErrorCode::MethodNotAllowed,
        StatusCode::CONFLICT => ErrorCode::Conflict,
        StatusCode::LENGTH_REQUIRED => ErrorCode::LengthRequired,
        StatusCode::PAYLOAD_TOO_LARGE => ErrorCode::PayloadTooLarge,
        StatusCode::URI_TOO_LONG => ErrorCode::UriTooLong,
        StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE => ErrorCode::RequestHeaderFieldsTooLarge,
        StatusCode::UNSUPPORTED_MEDIA_TYPE => ErrorCode::UnsupportedMediaType,
        StatusCode::UNPROCESSABLE_ENTITY => ErrorCode::UnprocessableEntity,
        StatusCode::UPGRADE_REQUIRED => ErrorCode::UpgradeRequired,
        StatusCode::REQUEST_TIMEOUT => ErrorCode::RequestTimeout,
        StatusCode::TOO_MANY_REQUESTS => ErrorCode::TooManyRequests,
        StatusCode::SERVICE_UNAVAILABLE => ErrorCode::ServiceUnavailable,
        StatusCode::INTERNAL_SERVER_ERROR => ErrorCode::InternalServerError,
        status if status.is_client_error() => ErrorCode::BadRequest,
        status if status.is_server_error() => ErrorCode::InternalServerError,
        _ => return None,
    };

    if response_has_json_content_type(response) {
        return None;
    }

    Some(JsonErrorResponse::from_public_error(
        PublicHttpError::from_status_with_code(response.status(), error_code),
    ))
}

fn response_has_json_content_type(response: &Response) -> bool {
    match response.headers().get(header::CONTENT_TYPE) {
        Some(content_type) => header_value_is_json_content_type(content_type),
        None => false,
    }
}

fn preserve_safe_protocol_headers(source: &Response, target: &mut Response) {
    if let Some(allow) = source.headers().get(header::ALLOW).cloned() {
        target.headers_mut().insert(header::ALLOW, allow);
    }
    if let Some(challenge) = source.headers().get(header::WWW_AUTHENTICATE).cloned() {
        target
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, challenge);
    }
    if let Some(retry_after) = source.headers().get(header::RETRY_AFTER).cloned() {
        target
            .headers_mut()
            .insert(header::RETRY_AFTER, retry_after);
    }
}
