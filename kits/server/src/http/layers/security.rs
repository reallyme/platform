// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

mod header_limits;
use header_limits::header_limits_rejection_reason;

mod proxy_metadata;
use proxy_metadata::{
    direct_request_without_https_proof_allowed, forwarded_client_ip_from_headers,
    normalized_external_host, normalized_external_proto, request_contains_proxy_headers,
    strip_untrusted_proxy_headers,
};

#[path = "security/request_policy.rs"]
mod request_policy;
use request_policy::{
    host_authority_is_allowed, is_operational_route, operational_route_is_blocked,
    peer_ip_from_request, record_security_rejection, route_template_for_request,
};

use std::convert::Infallible;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, Request};
use axum::response::{IntoResponse, Response};
use pin_project_lite::pin_project;
use tower::{Layer, Service};

use crate::config::{HostAuthority, HttpSecurityConfig, NetworkPort, SecurityHeadersConfig};
use crate::observability::{HttpMethodLabel, HttpRejectionReason};

use super::super::response::JsonErrorResponse;
use super::super::{ErrorCode, PublicHttpError, request_id_from_headers};
use super::listener::listener_name_for_request;

const FORWARDED_HEADER: HeaderName = HeaderName::from_static("forwarded");
const X_FORWARDED_FOR_HEADER: HeaderName = HeaderName::from_static("x-forwarded-for");
const X_FORWARDED_HOST_HEADER: HeaderName = HeaderName::from_static("x-forwarded-host");
const X_FORWARDED_PORT_HEADER: HeaderName = HeaderName::from_static("x-forwarded-port");
const X_FORWARDED_PROTO_HEADER: HeaderName = HeaderName::from_static("x-forwarded-proto");
const X_REAL_IP_HEADER: HeaderName = HeaderName::from_static("x-real-ip");
const X_CONTENT_TYPE_OPTIONS_HEADER: HeaderName = HeaderName::from_static("x-content-type-options");
const REFERRER_POLICY_HEADER: HeaderName = HeaderName::from_static("referrer-policy");
const X_FRAME_OPTIONS_HEADER: HeaderName = HeaderName::from_static("x-frame-options");
const CONTENT_SECURITY_POLICY_HEADER: HeaderName =
    HeaderName::from_static("content-security-policy");
const CROSS_ORIGIN_RESOURCE_POLICY_HEADER: HeaderName =
    HeaderName::from_static("cross-origin-resource-policy");
const CACHE_CONTROL_HEADER: HeaderName = HeaderName::from_static("cache-control");

const NOSNIFF_VALUE: HeaderValue = HeaderValue::from_static("nosniff");
const REFERRER_POLICY_VALUE: HeaderValue =
    HeaderValue::from_static("strict-origin-when-cross-origin");
const DENY_VALUE: HeaderValue = HeaderValue::from_static("DENY");
const FRAME_ANCESTORS_NONE_VALUE: HeaderValue = HeaderValue::from_static("frame-ancestors 'none'");
const SAME_SITE_VALUE: HeaderValue = HeaderValue::from_static("same-site");
const NO_STORE_VALUE: HeaderValue = HeaderValue::from_static("no-store");

/// Normalized client IP extracted from trusted proxy metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForwardedClientIp(IpAddr);

impl ForwardedClientIp {
    /// Returns the normalized client IP.
    pub const fn into_ip_addr(self) -> IpAddr {
        self.0
    }
}

/// Normalized external host authority selected from trusted proxy metadata or the direct request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardedHost(HostAuthority);

impl ForwardedHost {
    /// Constructs normalized host metadata from a validated authority.
    pub const fn new(authority: HostAuthority) -> Self {
        Self(authority)
    }

    /// Returns the validated external authority.
    pub const fn authority(&self) -> &HostAuthority {
        &self.0
    }

    /// Returns the normalized external host name or IP literal.
    pub fn host(&self) -> &str {
        self.0.host()
    }

    /// Returns the explicit external port, if one was present.
    pub const fn port(&self) -> Option<NetworkPort> {
        self.0.port()
    }
}

/// Normalized external scheme selected from trusted proxy metadata or the direct request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardedProto {
    /// HTTP.
    Http,
    /// HTTPS.
    Https,
}

impl ForwardedProto {
    /// Returns the stable lowercase scheme string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    /// Returns whether the normalized external scheme is HTTPS.
    pub const fn is_https(self) -> bool {
        matches!(self, Self::Https)
    }
}

/// Safely-derived external request origin assembled from normalized scheme and authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRequestOrigin {
    host: ForwardedHost,
    proto: ForwardedProto,
}

impl ExternalRequestOrigin {
    /// Constructs a normalized external origin.
    pub const fn new(host: ForwardedHost, proto: ForwardedProto) -> Self {
        Self { host, proto }
    }

    /// Returns the normalized external authority.
    pub const fn host(&self) -> &ForwardedHost {
        &self.host
    }

    /// Returns the normalized external scheme.
    pub const fn proto(&self) -> ForwardedProto {
        self.proto
    }

    /// Returns the explicit external port, if one was present.
    pub const fn port(&self) -> Option<NetworkPort> {
        self.host.port()
    }

    /// Returns the safely-derived base URL for the external request origin.
    pub fn base_url(&self) -> String {
        format!(
            "{}://{}",
            self.proto.as_str(),
            self.host.authority().as_str()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProxyMetadataError {
    InvalidForwardedHost,
    InvalidForwardedProto,
    ConflictingForwardedHost,
    ConflictingForwardedProto,
    InsecureExternalScheme,
}

impl ProxyMetadataError {
    const fn rejection_reason(self) -> HttpRejectionReason {
        match self {
            Self::InvalidForwardedHost => HttpRejectionReason::MalformedForwardedHost,
            Self::InvalidForwardedProto => HttpRejectionReason::MalformedForwardedProto,
            Self::ConflictingForwardedHost => HttpRejectionReason::ConflictingForwardedHost,
            Self::ConflictingForwardedProto => HttpRejectionReason::ConflictingForwardedProto,
            Self::InsecureExternalScheme => HttpRejectionReason::InsecureExternalScheme,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForwardedMetadataDebugReason {
    ConflictingForwardedHost,
    ConflictingForwardedProto,
    InvalidFallbackForwardedHost,
    InvalidFallbackForwardedProto,
}

impl ForwardedMetadataDebugReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ConflictingForwardedHost => "conflicting_forwarded_host",
            Self::ConflictingForwardedProto => "conflicting_forwarded_proto",
            Self::InvalidFallbackForwardedHost => "invalid_fallback_forwarded_host",
            Self::InvalidFallbackForwardedProto => "invalid_fallback_forwarded_proto",
        }
    }
}

/// Layer that applies generic HTTP hardening before app handlers run.
#[derive(Debug, Clone)]
pub struct HttpSecurityLayer {
    config: HttpSecurityConfig,
}

/// Creates middleware for generic HTTP security posture.
pub fn security_layer(config: &HttpSecurityConfig) -> HttpSecurityLayer {
    HttpSecurityLayer {
        config: config.clone(),
    }
}

impl<S> Layer<S> for HttpSecurityLayer {
    type Service = HttpSecurityService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HttpSecurityService {
            inner,
            config: self.config.clone(),
        }
    }
}

#[derive(Clone)]
pub struct HttpSecurityService<S> {
    inner: S,
    config: HttpSecurityConfig,
}

impl<S> Service<Request<Body>> for HttpSecurityService<S>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = HttpSecurityResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let method = HttpMethodLabel::from_method(request.method());
        let request_id = request_id_from_headers(request.headers());
        // Enforce limits before normalization can strip attacker-supplied headers.
        if let Some(reason) =
            header_limits_rejection_reason(request.headers(), self.config.header_limits())
        {
            let request_id = request_id_from_headers(request.headers());
            record_security_rejection(&request, method, reason);
            return HttpSecurityResponseFuture::ready(
                JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                    ErrorCode::RequestHeaderFieldsTooLarge,
                ))
                .with_optional_request_id(request_id)
                .into_response(),
                self.config.security_headers(),
            );
        }

        let peer_ip = peer_ip_from_request(&request);
        let contains_proxy_headers = request_contains_proxy_headers(&request);
        let trusted_peer = self.config.trusted_proxy_headers().trusts_peer(peer_ip);
        let proxy_request_metadata = self.config.trusted_proxy_request_metadata();
        let strict_forwarded_header_consistency =
            proxy_request_metadata.strict_forwarded_header_consistency();
        let normalized_host = match normalized_external_host(
            &request,
            trusted_peer,
            proxy_request_metadata.trusted_forwarded_host(),
            strict_forwarded_header_consistency,
            listener_name_for_request(&request).as_str(),
            route_template_for_request(&request),
        ) {
            Ok(host) => host,
            Err(error) => {
                record_security_rejection(&request, method, error.rejection_reason());
                return HttpSecurityResponseFuture::ready(
                    JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                        ErrorCode::BadRequest,
                    ))
                    .with_optional_request_id(request_id)
                    .into_response(),
                    self.config.security_headers(),
                );
            }
        };
        let normalized_proto = match normalized_external_proto(
            &request,
            trusted_peer,
            proxy_request_metadata.trusted_forwarded_proto(),
            strict_forwarded_header_consistency,
            listener_name_for_request(&request).as_str(),
            route_template_for_request(&request),
        ) {
            Ok(proto) => proto,
            Err(error) => {
                record_security_rejection(&request, method, error.rejection_reason());
                return HttpSecurityResponseFuture::ready(
                    JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                        ErrorCode::BadRequest,
                    ))
                    .with_optional_request_id(request_id)
                    .into_response(),
                    self.config.security_headers(),
                );
            }
        };

        if !host_authority_is_allowed(
            normalized_host.as_ref(),
            self.config.host_authority_policy(),
        ) {
            record_security_rejection(&request, method, HttpRejectionReason::BlockedHostAuthority);
            return HttpSecurityResponseFuture::ready(
                JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                    ErrorCode::BadRequest,
                ))
                .with_optional_request_id(request_id)
                .into_response(),
                self.config.security_headers(),
            );
        }

        if proxy_request_metadata.require_https_external_scheme() {
            let proxied_request = trusted_peer && contains_proxy_headers;
            let direct_request_allowed =
                direct_request_without_https_proof_allowed(&request, peer_ip);

            if proxied_request && !normalized_proto.is_some_and(ForwardedProto::is_https)
                || (!proxied_request && !direct_request_allowed)
            {
                record_security_rejection(
                    &request,
                    method,
                    ProxyMetadataError::InsecureExternalScheme.rejection_reason(),
                );
                return HttpSecurityResponseFuture::ready(
                    JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                        ErrorCode::BadRequest,
                    ))
                    .with_optional_request_id(request_id)
                    .into_response(),
                    self.config.security_headers(),
                );
            }
        }

        if trusted_peer {
            if let Some(client_ip) = forwarded_client_ip_from_headers(request.headers()) {
                request.extensions_mut().insert(client_ip);
            }
        } else if contains_proxy_headers {
            record_security_rejection(&request, method, HttpRejectionReason::UntrustedProxyHeaders);
        }

        if let Some(host) = normalized_host.clone() {
            request.extensions_mut().insert(host.clone());
            if let Some(proto) = normalized_proto {
                request
                    .extensions_mut()
                    .insert(ExternalRequestOrigin::new(host, proto));
            }
        }

        if let Some(proto) = normalized_proto {
            request.extensions_mut().insert(proto);
        }

        if proxy_request_metadata.strip_raw_proxy_headers() {
            strip_untrusted_proxy_headers(&mut request);
        }

        if operational_route_is_blocked(&request, self.config.operational_route_access()) {
            let request_id = request_id_from_headers(request.headers());
            record_security_rejection(
                &request,
                method,
                HttpRejectionReason::BlockedOperationalRoute,
            );
            return HttpSecurityResponseFuture::ready(
                JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                    ErrorCode::Forbidden,
                ))
                .with_optional_request_id(request_id)
                .into_response(),
                self.config.security_headers(),
            );
        }

        HttpSecurityResponseFuture::inner(self.inner.call(request), self.config.security_headers())
    }
}

pin_project! {
    /// Response future for [`HttpSecurityService`].
    pub struct HttpSecurityResponseFuture<F> {
        #[pin]
        state: HttpSecurityResponseFutureState<F>,
        security_headers: SecurityHeadersConfig,
    }
}

pin_project! {
    #[project = HttpSecurityResponseFutureStateProj]
    enum HttpSecurityResponseFutureState<F> {
        Inner {
            #[pin]
            inner: F,
        },
        Ready {
            response: Option<Response>,
        },
    }
}

impl<F> HttpSecurityResponseFuture<F> {
    fn inner(inner: F, security_headers: SecurityHeadersConfig) -> Self {
        Self {
            state: HttpSecurityResponseFutureState::Inner { inner },
            security_headers,
        }
    }

    fn ready(response: Response, security_headers: SecurityHeadersConfig) -> Self {
        Self {
            state: HttpSecurityResponseFutureState::Ready {
                response: Some(response),
            },
            security_headers,
        }
    }
}

impl<F> Future for HttpSecurityResponseFuture<F>
where
    F: Future<Output = Result<Response, Infallible>>,
{
    type Output = Result<Response, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let mut response = match this.state.project() {
            HttpSecurityResponseFutureStateProj::Inner { inner } => ready!(inner.poll(cx))?,
            HttpSecurityResponseFutureStateProj::Ready { response } => match response.take() {
                Some(response) => response,
                None => JsonErrorResponse::internal_server_error().into_response(),
            },
        };

        apply_security_headers(&mut response, *this.security_headers);

        Poll::Ready(Ok(response))
    }
}

fn apply_security_headers(response: &mut Response, config: SecurityHeadersConfig) {
    if !config.enabled() {
        return;
    }

    let should_disable_error_caching =
        response.status().is_client_error() || response.status().is_server_error();
    let headers = response.headers_mut();
    headers
        .entry(X_CONTENT_TYPE_OPTIONS_HEADER)
        .or_insert(NOSNIFF_VALUE);
    headers
        .entry(REFERRER_POLICY_HEADER)
        .or_insert(REFERRER_POLICY_VALUE);
    headers.entry(X_FRAME_OPTIONS_HEADER).or_insert(DENY_VALUE);
    headers
        .entry(CONTENT_SECURITY_POLICY_HEADER)
        .or_insert(FRAME_ANCESTORS_NONE_VALUE);
    headers
        .entry(CROSS_ORIGIN_RESOURCE_POLICY_HEADER)
        .or_insert(SAME_SITE_VALUE);

    if should_disable_error_caching {
        headers
            .entry(CACHE_CONTROL_HEADER)
            .or_insert(NO_STORE_VALUE);
    }
}

#[cfg(test)]
#[path = "security/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "security/header_limit_tests.rs"]
mod header_limit_tests;
