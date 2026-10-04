// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::Infallible;
use std::error::Error as StdError;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, ready};

use axum::body::{Body, to_bytes};
use axum::extract::MatchedPath;
use axum::extract::connect_info::ConnectInfo;
use axum::http::Request;
use axum::http::header::CONTENT_LENGTH;
use axum::response::{IntoResponse, Response};
use http_body_util::LengthLimitError;
use metrics::SharedString;
use pin_project_lite::pin_project;
use tower::{Layer, Service};

use crate::http::{
    ErrorCode, HttpListenerIdentity, HttpListenerName, HttpListenerVisibility,
    HttpRateLimitTierName, HttpRouteVisibilityPolicy, PublicHttpError, RequestBodyLimitBytes,
    request_id_from_headers,
};
use crate::observability::{
    HttpMethodLabel, HttpRateLimitOutcome, HttpRejectionReason, MetricRouteTemplateLabel,
    TransportLabel, record_http_rate_limit_decision_for_route_template_with_transport,
    record_http_request_rejected_for_route_template_with_transport,
};
use crate::runtime::HttpRateLimitTierPolicy;
use crate::runtime::RateLimitDecision;
use crate::runtime::{RateLimitRegistry, RateLimitSourceIdentity};

use super::super::response::JsonErrorResponse;

#[path = "route_visibility/policy_path.rs"]
mod policy_path;
use policy_path::canonical_policy_path;

/// Creates a route visibility guard for one listener.
pub fn route_visibility_layer(
    listener_name: HttpListenerName,
    listener_visibility: HttpListenerVisibility,
    listener_rate_limit_tier: Option<HttpRateLimitTierName>,
    rate_limit_policies: Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>>,
    policy: &HttpRouteVisibilityPolicy,
) -> RouteVisibilityLayer {
    route_visibility_layer_with_rate_limit_registry(
        listener_name,
        listener_visibility,
        listener_rate_limit_tier,
        Arc::new(RateLimitRegistry::new(rate_limit_policies)),
        policy,
    )
}

/// Creates a route visibility guard for one listener with an explicit limiter.
pub(crate) fn route_visibility_layer_with_rate_limit_registry(
    listener_name: HttpListenerName,
    listener_visibility: HttpListenerVisibility,
    listener_rate_limit_tier: Option<HttpRateLimitTierName>,
    rate_limit_registry: Arc<RateLimitRegistry>,
    policy: &HttpRouteVisibilityPolicy,
) -> RouteVisibilityLayer {
    RouteVisibilityLayer {
        listener_name,
        listener_visibility,
        listener_rate_limit_tier,
        rate_limit_registry,
        policy: policy.clone(),
    }
}

/// Layer that rejects requests whose route visibility does not allow this listener.
#[derive(Debug, Clone)]
pub struct RouteVisibilityLayer {
    listener_name: HttpListenerName,
    listener_visibility: HttpListenerVisibility,
    listener_rate_limit_tier: Option<HttpRateLimitTierName>,
    rate_limit_registry: Arc<RateLimitRegistry>,
    policy: HttpRouteVisibilityPolicy,
}

impl<S> Layer<S> for RouteVisibilityLayer {
    type Service = RouteVisibilityService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RouteVisibilityService {
            inner,
            listener_name: self.listener_name.clone(),
            listener_visibility: self.listener_visibility.clone(),
            listener_rate_limit_tier: self.listener_rate_limit_tier.clone(),
            rate_limit_registry: Arc::clone(&self.rate_limit_registry),
            policy: self.policy.clone(),
        }
    }
}

/// Service that rejects listener/route visibility mismatches before app handlers run.
#[derive(Clone)]
pub struct RouteVisibilityService<S> {
    inner: S,
    listener_name: HttpListenerName,
    listener_visibility: HttpListenerVisibility,
    listener_rate_limit_tier: Option<HttpRateLimitTierName>,
    rate_limit_registry: Arc<RateLimitRegistry>,
    policy: HttpRouteVisibilityPolicy,
}

impl<S> Service<Request<Body>> for RouteVisibilityService<S>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = RouteVisibilityResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let matched_path = request.extensions().get::<MatchedPath>();
        let route_template = MetricRouteTemplateLabel::from_matched_path(matched_path);

        // MatchedPath is a route template; policy must evaluate the concrete
        // request path so wildcard and parameter routes cannot hide children.
        let request_path = request.uri().path();
        let canonical_path = match canonical_policy_path(request_path) {
            Some(path) => path,
            None => {
                let request_id = request_id_from_headers(request.headers());
                record_http_request_rejected_for_route_template_with_transport(
                    SharedString::from_shared(self.listener_name.clone_shared()),
                    transport_label_for_request(&request),
                    HttpMethodLabel::from_method(request.method()),
                    &route_template,
                    HttpRejectionReason::BlockedRouteVisibility,
                );
                return RouteVisibilityResponseFuture::ready(
                    JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                        ErrorCode::BadRequest,
                    ))
                    .with_optional_request_id(request_id)
                    .into_response(),
                );
            }
        };

        let matching_rule = self.policy.matching_rule_for_request(
            &self.listener_name,
            &self.listener_visibility,
            canonical_path.as_ref(),
        );
        // Axum selects handlers using the raw URI. A decoded spelling may
        // select a more permissive exact rule than the handler actually uses.
        let raw_matching_rule = self.policy.matching_rule_for_request(
            &self.listener_name,
            &self.listener_visibility,
            request_path,
        );
        if raw_matching_rule != matching_rule
            || !self.policy.allows_request(
                &self.listener_name,
                &self.listener_visibility,
                request_path,
            )
            || !self.policy.allows_request(
                &self.listener_name,
                &self.listener_visibility,
                canonical_path.as_ref(),
            )
        {
            let request_id = request_id_from_headers(request.headers());
            record_http_request_rejected_for_route_template_with_transport(
                SharedString::from_shared(self.listener_name.clone_shared()),
                transport_label_for_request(&request),
                HttpMethodLabel::from_method(request.method()),
                &route_template,
                HttpRejectionReason::BlockedRouteVisibility,
            );

            return RouteVisibilityResponseFuture::ready(
                JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                    ErrorCode::Forbidden,
                ))
                .with_optional_request_id(request_id)
                .into_response(),
            );
        }

        let request_body_limit = matching_rule.and_then(|rule| rule.request_body_limit());
        if let Some((status, reason)) =
            request_body_limit.and_then(|limit| request_body_limit_rejection(&request, limit))
        {
            let request_id = request_id_from_headers(request.headers());
            record_http_request_rejected_for_route_template_with_transport(
                SharedString::from_shared(self.listener_name.clone_shared()),
                transport_label_for_request(&request),
                HttpMethodLabel::from_method(request.method()),
                &route_template,
                reason,
            );
            return RouteVisibilityResponseFuture::ready(
                JsonErrorResponse::from_public_error(PublicHttpError::from_code(status))
                    .with_optional_request_id(request_id)
                    .into_response(),
            );
        }

        // Operational probes must remain observable while a public listener
        // is saturated. A route-specific policy can still opt them into a
        // dedicated tier without sharing the public request bucket.
        let rate_limit_tier = matching_rule
            .and_then(|rule| rule.rate_limit_tier())
            .or_else(|| {
                (!matches!(request_path, "/healthz" | "/readyz"))
                    .then_some(self.listener_rate_limit_tier.as_ref())
                    .flatten()
            });
        if let Some(rate_limit_tier) = rate_limit_tier {
            let source_identity = request_source_identity(&request);
            match self
                .rate_limit_registry
                .allow(rate_limit_tier, source_identity)
            {
                RateLimitDecision::Allowed => {}
                RateLimitDecision::SourceLimitReached => {
                    let request_id = request_id_from_headers(request.headers());
                    record_http_rate_limit_decision_for_route_template_with_transport(
                        SharedString::from_shared(self.listener_name.clone_shared()),
                        transport_label_for_request(&request),
                        HttpMethodLabel::from_method(request.method()),
                        &route_template,
                        SharedString::from_shared(rate_limit_tier.clone_shared()),
                        HttpRateLimitOutcome::Limited,
                    );
                    record_http_request_rejected_for_route_template_with_transport(
                        SharedString::from_shared(self.listener_name.clone_shared()),
                        transport_label_for_request(&request),
                        HttpMethodLabel::from_method(request.method()),
                        &route_template,
                        HttpRejectionReason::RateLimited,
                    );
                    return RouteVisibilityResponseFuture::ready(
                        JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                            ErrorCode::TooManyRequests,
                        ))
                        .with_optional_request_id(request_id)
                        .into_response(),
                    );
                }
                RateLimitDecision::RegistryFull => {
                    let request_id = request_id_from_headers(request.headers());
                    record_http_rate_limit_decision_for_route_template_with_transport(
                        SharedString::from_shared(self.listener_name.clone_shared()),
                        transport_label_for_request(&request),
                        HttpMethodLabel::from_method(request.method()),
                        &route_template,
                        SharedString::from_shared(rate_limit_tier.clone_shared()),
                        HttpRateLimitOutcome::RegistryFull,
                    );
                    record_http_request_rejected_for_route_template_with_transport(
                        SharedString::from_shared(self.listener_name.clone_shared()),
                        transport_label_for_request(&request),
                        HttpMethodLabel::from_method(request.method()),
                        &route_template,
                        HttpRejectionReason::RateLimited,
                    );
                    return RouteVisibilityResponseFuture::ready(
                        JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                            ErrorCode::TooManyRequests,
                        ))
                        .with_optional_request_id(request_id)
                        .into_response(),
                    );
                }
            }

            record_http_rate_limit_decision_for_route_template_with_transport(
                SharedString::from_shared(self.listener_name.clone_shared()),
                transport_label_for_request(&request),
                HttpMethodLabel::from_method(request.method()),
                &route_template,
                SharedString::from_shared(rate_limit_tier.clone_shared()),
                HttpRateLimitOutcome::Allowed,
            );
            request.extensions_mut().insert(rate_limit_tier.clone());
        }

        let local_socket_addr = request
            .extensions()
            .get::<HttpListenerIdentity>()
            .and_then(HttpListenerIdentity::local_socket_addr);
        request.extensions_mut().insert(HttpListenerIdentity::new(
            self.listener_name.clone(),
            self.listener_visibility.clone(),
            local_socket_addr,
        ));

        if let Some(limit) = request_body_limit {
            let mut inner = self.inner.clone();
            let request_id = request_id_from_headers(request.headers());
            let future = async move {
                let (parts, body) = request.into_parts();
                let bytes = match to_bytes(body, limit.as_usize()).await {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        let code = if error
                            .source()
                            .is_some_and(|source| source.is::<LengthLimitError>())
                        {
                            ErrorCode::PayloadTooLarge
                        } else {
                            ErrorCode::BadRequest
                        };
                        return Ok(JsonErrorResponse::from_public_error(
                            PublicHttpError::from_code(code),
                        )
                        .with_optional_request_id(request_id)
                        .into_response());
                    }
                };
                let request = Request::from_parts(parts, Body::from(bytes));
                std::future::poll_fn(|cx| inner.poll_ready(cx)).await?;
                inner.call(request).await
            };
            return RouteVisibilityResponseFuture::buffered(Box::pin(future));
        }

        RouteVisibilityResponseFuture::inner(self.inner.call(request))
    }
}

fn request_body_limit_rejection(
    request: &Request<Body>,
    request_body_limit: RequestBodyLimitBytes,
) -> Option<(ErrorCode, HttpRejectionReason)> {
    let mut values = request.headers().get_all(CONTENT_LENGTH).iter();
    let value = values.next()?;

    if values.next().is_some() {
        return Some((
            ErrorCode::BadRequest,
            HttpRejectionReason::MalformedContentLength,
        ));
    }

    let Ok(value) = value.to_str() else {
        return Some((
            ErrorCode::BadRequest,
            HttpRejectionReason::MalformedContentLength,
        ));
    };

    let Ok(content_length) = value.parse::<usize>() else {
        return Some((
            ErrorCode::BadRequest,
            HttpRejectionReason::MalformedContentLength,
        ));
    };

    (content_length > request_body_limit.as_usize()).then_some((
        ErrorCode::PayloadTooLarge,
        HttpRejectionReason::PayloadTooLarge,
    ))
}

fn request_source_identity(request: &Request<Body>) -> RateLimitSourceIdentity {
    // This listener policy runs before app authentication. Per-principal
    // quotas belong in an authenticated app adapter, not a transport guard.
    if let Some(forwarded_ip) = request_source_forwarded_ip(request) {
        return RateLimitSourceIdentity::ForwardedIp(forwarded_ip);
    }

    if let Some(peer_ip) = request_peer_ip(request) {
        return RateLimitSourceIdentity::PeerIp(peer_ip);
    }

    RateLimitSourceIdentity::Anonymous
}

fn request_source_forwarded_ip(request: &Request<Body>) -> Option<IpAddr> {
    request
        .extensions()
        .get::<crate::http::ForwardedClientIp>()
        .map(|client_ip| (*client_ip).into_ip_addr())
}

fn request_peer_ip(request: &Request<Body>) -> Option<IpAddr> {
    request
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|connect_info| connect_info.0.ip())
}

pin_project! {
    /// Response future for [`RouteVisibilityService`].
    pub struct RouteVisibilityResponseFuture<F> {
        #[pin]
        state: RouteVisibilityResponseFutureState<F>,
    }
}

pin_project! {
    #[project = RouteVisibilityResponseFutureStateProj]
    enum RouteVisibilityResponseFutureState<F> {
        Inner {
            #[pin]
            inner: F,
        },
        Ready {
            response: Option<Response>,
        },
        Buffered {
            #[pin]
            future: Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>,
        },
    }
}

impl<F> RouteVisibilityResponseFuture<F> {
    fn inner(inner: F) -> Self {
        Self {
            state: RouteVisibilityResponseFutureState::Inner { inner },
        }
    }

    fn ready(response: Response) -> Self {
        Self {
            state: RouteVisibilityResponseFutureState::Ready {
                response: Some(response),
            },
        }
    }

    fn buffered(
        future: Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>,
    ) -> Self {
        Self {
            state: RouteVisibilityResponseFutureState::Buffered { future },
        }
    }
}

impl<F> Future for RouteVisibilityResponseFuture<F>
where
    F: Future<Output = Result<Response, Infallible>>,
{
    type Output = Result<Response, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let response = match this.state.project() {
            RouteVisibilityResponseFutureStateProj::Inner { inner } => ready!(inner.poll(cx))?,
            RouteVisibilityResponseFutureStateProj::Ready { response } => match response.take() {
                Some(response) => response,
                None => JsonErrorResponse::internal_server_error().into_response(),
            },
            RouteVisibilityResponseFutureStateProj::Buffered { future } => ready!(future.poll(cx))?,
        };

        Poll::Ready(Ok(response))
    }
}

fn transport_label_for_request(request: &Request<Body>) -> TransportLabel {
    let Some(content_type) = request
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    else {
        return TransportLabel::Http;
    };

    if content_type.starts_with("application/connect+") {
        return TransportLabel::Connect;
    }

    TransportLabel::Http
}

#[cfg(test)]
mod body_limit_tests;
#[cfg(test)]
mod encoded_carve_out_tests;
#[cfg(test)]
mod path_canonicalization_tests;
#[cfg(test)]
mod route_visibility_tests;
