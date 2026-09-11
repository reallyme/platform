// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::Infallible;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, ready};

use axum::extract::connect_info::ConnectInfo;
use metrics::SharedString;
use pin_project_lite::pin_project;
use tonic::body::Body as TonicBody;
use tonic::codegen::http;
use tonic::codegen::http::header::CONTENT_LENGTH;
use tonic::codegen::http::{Request, Response, StatusCode};
use tower::{Layer, Service};

use crate::authn::Principal;
use crate::grpc::{GRPC_TIMEOUT_METADATA_KEY, GrpcTimeout};
use crate::http::ForwardedClientIp;
use crate::observability::{
    HttpMethodLabel, HttpRateLimitOutcome, HttpRejectionReason, MetricRouteTemplateLabel,
    TransportLabel, record_http_rate_limit_decision_for_route_template_with_transport,
    record_http_request_rejected_for_route_template_with_transport,
};
use crate::runtime::{
    GrpcMethodPolicy, RateLimitDecision, RateLimitRegistry, RateLimitSourceIdentity,
};

const GRPC_CONTENT_TYPE: &str = "application/grpc";
const GRPC_STATUS_RESOURCE_EXHAUSTED: &str = "8";
const GRPC_STATUS_INTERNAL: &str = "13";
const UNKNOWN_GRPC_METHOD_ROUTE: &str = "/__unknown_grpc_method";

/// Native gRPC policy bundle enforced before app handlers run.
#[derive(Clone)]
pub struct GrpcPolicy {
    listener_name: Arc<str>,
    max_timeout: Option<GrpcTimeout>,
    deadline_required: bool,
    method_policies: Arc<Vec<GrpcMethodPolicy>>,
    rate_limit_registry: Arc<RateLimitRegistry>,
}

impl GrpcPolicy {
    /// Creates a gRPC policy from validated server composition.
    pub fn new(
        listener_name: Arc<str>,
        max_timeout: Option<GrpcTimeout>,
        deadline_required: bool,
        method_policies: Vec<GrpcMethodPolicy>,
        rate_limit_registry: Arc<RateLimitRegistry>,
    ) -> Self {
        Self {
            listener_name,
            max_timeout,
            deadline_required,
            method_policies: Arc::new(method_policies),
            rate_limit_registry,
        }
    }
}

/// Wraps a tonic service with native-gRPC deadline/size/rate policy checks.
///
/// IMPORTANT:
/// - Authoritative inbound message-size enforcement is tonic decode limits
///   (`max_decoding_message_bytes`) configured on the server/service.
/// - `max_request_message_bytes` here is an early/advisory guard based on
///   `Content-Length` and is not full streaming byte accounting.
pub fn grpc_policy_layer(policy: GrpcPolicy) -> GrpcPolicyLayer {
    GrpcPolicyLayer { policy }
}

#[derive(Clone)]
pub struct GrpcPolicyLayer {
    policy: GrpcPolicy,
}

impl<S> Layer<S> for GrpcPolicyLayer {
    type Service = GrpcPolicyService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcPolicyService {
            inner,
            policy: self.policy.clone(),
        }
    }
}

#[derive(Clone)]
pub struct GrpcPolicyService<S> {
    inner: S,
    policy: GrpcPolicy,
}

impl<S> Service<Request<TonicBody>> for GrpcPolicyService<S>
where
    S: Service<Request<TonicBody>, Response = Response<TonicBody>, Error = Infallible>,
    S::Future: Send + 'static,
{
    type Response = Response<TonicBody>;
    type Error = Infallible;
    type Future = GrpcPolicyResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<TonicBody>) -> Self::Future {
        let method_path = request.uri().path();
        let matching_policy = self
            .policy
            .method_policies
            .iter()
            .find(|policy| policy.method() == method_path);
        let route_template = matching_policy
            .map(|policy| MetricRouteTemplateLabel::from_runtime_route_template(policy.method()))
            .unwrap_or_else(|| {
                MetricRouteTemplateLabel::from_runtime_route_template(UNKNOWN_GRPC_METHOD_ROUTE)
            });
        let listener_name = SharedString::from_shared(Arc::clone(&self.policy.listener_name));

        if self.policy.deadline_required
            && request.headers().get(GRPC_TIMEOUT_METADATA_KEY).is_none()
        {
            return GrpcPolicyResponseFuture::ready(grpc_resource_exhausted_response(
                "deadline_required",
            ));
        }

        if let Some(max_timeout) = self.policy.max_timeout
            && let Some(timeout) = request
                .headers()
                .get(GRPC_TIMEOUT_METADATA_KEY)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_grpc_timeout_header)
            && timeout > max_timeout
        {
            return GrpcPolicyResponseFuture::ready(grpc_resource_exhausted_response(
                "deadline_exceeds_max",
            ));
        }

        if let Some(max_request_message_bytes) =
            matching_policy.and_then(GrpcMethodPolicy::max_request_message_bytes)
            && request_exceeds_body_limit(&request, max_request_message_bytes)
        {
            record_http_request_rejected_for_route_template_with_transport(
                listener_name,
                TransportLabel::Grpc,
                HttpMethodLabel::Post,
                &route_template,
                HttpRejectionReason::MessageTooLarge,
            );
            return GrpcPolicyResponseFuture::ready(grpc_resource_exhausted_response(
                "message_too_large",
            ));
        }

        if let Some(rate_limit_tier) = matching_policy.and_then(GrpcMethodPolicy::rate_limit_tier) {
            let source_identity = request_source_identity(&request);
            let decision = self
                .policy
                .rate_limit_registry
                .allow(rate_limit_tier, source_identity);
            let outcome = match decision {
                RateLimitDecision::Allowed => None,
                RateLimitDecision::SourceLimitReached => Some(HttpRateLimitOutcome::Limited),
                RateLimitDecision::RegistryFull => Some(HttpRateLimitOutcome::RegistryFull),
            };
            if let Some(outcome) = outcome {
                record_http_rate_limit_decision_for_route_template_with_transport(
                    listener_name.clone(),
                    TransportLabel::Grpc,
                    HttpMethodLabel::Post,
                    &route_template,
                    SharedString::from_shared(rate_limit_tier.clone_shared()),
                    outcome,
                );
                record_http_request_rejected_for_route_template_with_transport(
                    listener_name,
                    TransportLabel::Grpc,
                    HttpMethodLabel::Post,
                    &route_template,
                    HttpRejectionReason::RateLimited,
                );
                return GrpcPolicyResponseFuture::ready(grpc_resource_exhausted_response(
                    "rate_limited",
                ));
            }
            record_http_rate_limit_decision_for_route_template_with_transport(
                listener_name,
                TransportLabel::Grpc,
                HttpMethodLabel::Post,
                &route_template,
                SharedString::from_shared(rate_limit_tier.clone_shared()),
                HttpRateLimitOutcome::Allowed,
            );
        }

        GrpcPolicyResponseFuture::inner(self.inner.call(request))
    }
}

pin_project! {
    /// Response future for [`GrpcPolicyService`].
    pub struct GrpcPolicyResponseFuture<F> {
        #[pin]
        state: GrpcPolicyResponseFutureState<F>,
    }
}

pin_project! {
    #[project = GrpcPolicyResponseFutureStateProj]
    enum GrpcPolicyResponseFutureState<F> {
        Inner {
            #[pin]
            inner: F,
        },
        Ready {
            response: Option<Response<TonicBody>>,
        },
    }
}

impl<F> GrpcPolicyResponseFuture<F> {
    fn inner(inner: F) -> Self {
        Self {
            state: GrpcPolicyResponseFutureState::Inner { inner },
        }
    }

    fn ready(response: Response<TonicBody>) -> Self {
        Self {
            state: GrpcPolicyResponseFutureState::Ready {
                response: Some(response),
            },
        }
    }
}

impl<F> Future for GrpcPolicyResponseFuture<F>
where
    F: Future<Output = Result<Response<TonicBody>, Infallible>>,
{
    type Output = Result<Response<TonicBody>, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let response = match this.state.project() {
            GrpcPolicyResponseFutureStateProj::Inner { inner } => ready!(inner.poll(cx))?,
            GrpcPolicyResponseFutureStateProj::Ready { response } => response
                .take()
                .unwrap_or_else(|| grpc_status_response(GRPC_STATUS_INTERNAL, "internal")),
        };
        Poll::Ready(Ok(response))
    }
}

fn grpc_resource_exhausted_response(message: &'static str) -> Response<TonicBody> {
    grpc_status_response(GRPC_STATUS_RESOURCE_EXHAUSTED, message)
}

fn grpc_status_response(status: &'static str, message: &'static str) -> Response<TonicBody> {
    let mut response = Response::new(TonicBody::empty());
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static(GRPC_CONTENT_TYPE),
    );
    response.headers_mut().insert(
        http::HeaderName::from_static("grpc-status"),
        http::HeaderValue::from_static(status),
    );
    response.headers_mut().insert(
        http::HeaderName::from_static("grpc-message"),
        http::HeaderValue::from_static(message),
    );
    response
}

fn request_exceeds_body_limit(request: &Request<TonicBody>, max_bytes: usize) -> bool {
    // Early/advisory guard only: header-based and intentionally fail-open when
    // `Content-Length` is absent or malformed. Authoritative inbound size
    // enforcement is tonic decode limits.
    let Some(value) = request.headers().get(CONTENT_LENGTH) else {
        return false;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };
    let Ok(content_length) = value.parse::<usize>() else {
        return false;
    };
    content_length > max_bytes
}

fn request_source_identity(request: &Request<TonicBody>) -> RateLimitSourceIdentity<'_> {
    if let Some(Principal::Authenticated(principal)) = request.extensions().get::<Principal>() {
        return RateLimitSourceIdentity::Principal(principal.principal_id().as_str());
    }

    if let Some(forwarded_ip) = request_source_forwarded_ip(request) {
        return RateLimitSourceIdentity::ForwardedIp(forwarded_ip);
    }

    if let Some(peer_ip) = request_peer_ip(request) {
        return RateLimitSourceIdentity::PeerIp(peer_ip);
    }

    RateLimitSourceIdentity::Anonymous
}

fn request_source_forwarded_ip(request: &Request<TonicBody>) -> Option<IpAddr> {
    request
        .extensions()
        .get::<ForwardedClientIp>()
        .map(|client_ip| (*client_ip).into_ip_addr())
}

fn request_peer_ip(request: &Request<TonicBody>) -> Option<IpAddr> {
    request
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|connect_info| connect_info.0.ip())
}

fn parse_grpc_timeout_header(value: &str) -> Option<GrpcTimeout> {
    if value.len() < 2 || value.len() > 9 {
        return None;
    }
    let (digits, unit) = value.split_at(value.len() - 1);
    let amount = digits.parse::<u64>().ok()?;
    let duration = match unit {
        "H" => std::time::Duration::from_secs(amount.checked_mul(3600)?),
        "M" => std::time::Duration::from_secs(amount.checked_mul(60)?),
        "S" => std::time::Duration::from_secs(amount),
        "m" => std::time::Duration::from_millis(amount),
        "u" => std::time::Duration::from_micros(amount),
        "n" => std::time::Duration::from_nanos(amount),
        _ => return None,
    };
    GrpcTimeout::new(duration).ok()
}
#[cfg(test)]
mod tests {
    use crate::runtime::RateLimitRegistry;
    use std::convert::Infallible;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use tonic::body::Body as TonicBody;
    use tonic::codegen::http::{Request, Response};
    use tower::{Layer, Service, ServiceExt, service_fn};

    use super::{GrpcPolicy, grpc_policy_layer};
    use crate::runtime::GrpcMethodPolicy;

    #[tokio::test]
    async fn method_content_length_limit_rejects_early_with_resource_exhausted() {
        let called = Arc::new(AtomicBool::new(false));
        let called_inner = Arc::clone(&called);
        let inner = service_fn(move |_: Request<TonicBody>| {
            called_inner.store(true, Ordering::SeqCst);
            async move { Ok::<Response<TonicBody>, Infallible>(Response::new(TonicBody::empty())) }
        });

        let policy = GrpcPolicy::new(
            Arc::<str>::from("grpc-public"),
            None,
            false,
            vec![GrpcMethodPolicy::new(
                String::from("/reallyme.api.v1.ApiStatusService/GetStatus"),
                None,
                Some(16),
            )],
            Arc::new(RateLimitRegistry::new(Arc::new(Vec::new()))),
        );
        let mut service = grpc_policy_layer(policy).layer(inner);

        let request = Request::builder()
            .uri("/reallyme.api.v1.ApiStatusService/GetStatus")
            .header("content-length", "1024")
            .body(TonicBody::empty())
            .expect("valid grpc request");

        let response = service
            .ready()
            .await
            .expect("service should be ready")
            .call(request)
            .await
            .expect("policy response should succeed");

        assert_eq!(
            response
                .headers()
                .get("grpc-status")
                .expect("grpc-status header should be present"),
            "8"
        );
        assert_eq!(
            response
                .headers()
                .get("grpc-message")
                .expect("grpc-message header should be present"),
            "message_too_large"
        );
        assert!(
            !called.load(Ordering::SeqCst),
            "inner handler should not run for early advisory reject"
        );
    }
}
