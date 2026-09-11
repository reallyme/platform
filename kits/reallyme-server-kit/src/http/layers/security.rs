// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

mod header_limits;
use header_limits::header_limits_rejection_reason;

use std::convert::Infallible;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, header};
use axum::response::{IntoResponse, Response};
use pin_project_lite::pin_project;
use tower::{Layer, Service};
use tracing::debug;

use crate::config::{
    HostAuthority, HostAuthorityPolicy, HttpSecurityConfig, NetworkPort, OperationalRouteAccess,
    SecurityHeadersConfig,
};
use crate::observability::{
    HttpMethodLabel, HttpRejectionReason, MetricRouteTemplateLabel, UNKNOWN_ROUTE_TEMPLATE,
    record_http_request_rejected_for_route_template,
};

use super::super::response::JsonErrorResponse;
use super::super::routes::{HEALTHZ_PATH, METRICS_PATH, READYZ_PATH, VERSION_PATH};
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

fn route_template_for_request(request: &Request<Body>) -> &str {
    request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or(UNKNOWN_ROUTE_TEMPLATE)
}

fn record_security_rejection(
    request: &Request<Body>,
    method: HttpMethodLabel,
    reason: HttpRejectionReason,
) {
    let route_template =
        MetricRouteTemplateLabel::from_matched_path(request.extensions().get::<MatchedPath>());
    record_http_request_rejected_for_route_template(
        listener_name_for_request(request),
        method,
        &route_template,
        reason,
    );
}

fn peer_ip_from_request(request: &Request<Body>) -> Option<IpAddr> {
    request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|connect_info| connect_info.0.ip())
}

fn host_authority_is_allowed(
    authority: Option<&ForwardedHost>,
    policy: &HostAuthorityPolicy,
) -> bool {
    match authority {
        Some(authority) => policy.allows_normalized(authority.authority()),
        None => matches!(policy, HostAuthorityPolicy::Any),
    }
}

fn operational_route_is_blocked(request: &Request<Body>, access: OperationalRouteAccess) -> bool {
    if access == OperationalRouteAccess::Public || !is_operational_route(request.uri().path()) {
        return false;
    }

    request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .is_none_or(|connect_info| !ip_is_loopback(connect_info.0.ip()))
}

fn is_operational_route(path: &str) -> bool {
    matches!(
        path,
        HEALTHZ_PATH | READYZ_PATH | VERSION_PATH | METRICS_PATH
    )
}

fn ip_is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}

fn strip_untrusted_proxy_headers(request: &mut Request<Body>) {
    let headers = request.headers_mut();
    headers.remove(FORWARDED_HEADER);
    headers.remove(X_FORWARDED_FOR_HEADER);
    headers.remove(X_FORWARDED_HOST_HEADER);
    headers.remove(X_FORWARDED_PORT_HEADER);
    headers.remove(X_FORWARDED_PROTO_HEADER);
    headers.remove(X_REAL_IP_HEADER);
}

fn request_contains_proxy_headers(request: &Request<Body>) -> bool {
    let headers = request.headers();

    headers.contains_key(FORWARDED_HEADER)
        || headers.contains_key(X_FORWARDED_FOR_HEADER)
        || headers.contains_key(X_FORWARDED_HOST_HEADER)
        || headers.contains_key(X_FORWARDED_PORT_HEADER)
        || headers.contains_key(X_FORWARDED_PROTO_HEADER)
        || headers.contains_key(X_REAL_IP_HEADER)
}

fn normalized_external_host(
    request: &Request<Body>,
    trusted_peer: bool,
    trust_forwarded_host: bool,
    strict_forwarded_header_consistency: bool,
    listener_name: &str,
    route_template: &str,
) -> Result<Option<ForwardedHost>, ProxyMetadataError> {
    if trusted_peer && trust_forwarded_host {
        let forwarded = forwarded_host_from_forwarded_header(request.headers());
        let x_forwarded = forwarded_host_from_x_forwarded_headers(request.headers());

        if let Some(authority) = resolve_forwarded_host_precedence(
            forwarded,
            x_forwarded,
            strict_forwarded_header_consistency,
            listener_name,
            route_template,
        )? {
            return Ok(Some(authority));
        }
    }

    Ok(direct_host_authority(request).map(ForwardedHost::new))
}

fn normalized_external_proto(
    request: &Request<Body>,
    trusted_peer: bool,
    trust_forwarded_proto: bool,
    strict_forwarded_header_consistency: bool,
    listener_name: &str,
    route_template: &str,
) -> Result<Option<ForwardedProto>, ProxyMetadataError> {
    if trusted_peer && trust_forwarded_proto {
        let forwarded = forwarded_proto_from_forwarded_header(request.headers());
        let x_forwarded = forwarded_proto_from_x_forwarded_headers(request.headers());

        if let Some(proto) = resolve_forwarded_proto_precedence(
            forwarded,
            x_forwarded,
            strict_forwarded_header_consistency,
            listener_name,
            route_template,
        )? {
            return Ok(Some(proto));
        }
    }

    Ok(request.uri().scheme_str().and_then(ForwardedProto::parse))
}

impl ForwardedProto {
    fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "http" => Some(Self::Http),
            "https" => Some(Self::Https),
            _ => None,
        }
    }
}

fn direct_host_authority(request: &Request<Body>) -> Option<HostAuthority> {
    request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| HostAuthority::retain_authority(value).ok())
        .or_else(|| {
            request
                .uri()
                .authority()
                .and_then(|value| HostAuthority::retain_authority(value.as_str()).ok())
        })
}

fn forwarded_host_from_forwarded_header(
    headers: &HeaderMap,
) -> Result<Option<ForwardedHost>, ProxyMetadataError> {
    if let Some(value) = headers.get(FORWARDED_HEADER)
        && let Some(host) = forwarded_header_parameter(value, "host")
    {
        let authority = HostAuthority::retain_authority(host)
            .map_err(|_| ProxyMetadataError::InvalidForwardedHost)?;
        return Ok(Some(ForwardedHost::new(authority)));
    }

    Ok(None)
}

fn forwarded_host_from_x_forwarded_headers(
    headers: &HeaderMap,
) -> Result<Option<ForwardedHost>, ProxyMetadataError> {
    if let Some(value) = headers.get(X_FORWARDED_HOST_HEADER) {
        let authority = first_comma_separated_token(value)
            .ok_or(ProxyMetadataError::InvalidForwardedHost)
            .and_then(|host| forwarded_host_authority_from_x_forwarded_headers(headers, host))?;
        return Ok(Some(ForwardedHost::new(authority)));
    }

    Ok(None)
}

fn resolve_forwarded_host_precedence(
    forwarded: Result<Option<ForwardedHost>, ProxyMetadataError>,
    x_forwarded: Result<Option<ForwardedHost>, ProxyMetadataError>,
    strict_forwarded_header_consistency: bool,
    listener_name: &str,
    route_template: &str,
) -> Result<Option<ForwardedHost>, ProxyMetadataError> {
    match (forwarded, x_forwarded) {
        (Ok(Some(forwarded_host)), Ok(Some(x_forwarded_host))) => {
            if forwarded_host != x_forwarded_host {
                if strict_forwarded_header_consistency {
                    return Err(ProxyMetadataError::ConflictingForwardedHost);
                }

                debug_forwarded_metadata_reason(
                    ForwardedMetadataDebugReason::ConflictingForwardedHost,
                    listener_name,
                    route_template,
                );
            }

            Ok(Some(forwarded_host))
        }
        (Ok(Some(forwarded_host)), Err(_)) => {
            debug_forwarded_metadata_reason(
                ForwardedMetadataDebugReason::InvalidFallbackForwardedHost,
                listener_name,
                route_template,
            );
            Ok(Some(forwarded_host))
        }
        (Ok(Some(forwarded_host)), Ok(None)) => Ok(Some(forwarded_host)),
        (Err(error), _) => Err(error),
        (Ok(None), Ok(Some(x_forwarded_host))) => Ok(Some(x_forwarded_host)),
        (Ok(None), Err(error)) => Err(error),
        (Ok(None), Ok(None)) => Ok(None),
    }
}

fn forwarded_host_authority_from_x_forwarded_headers(
    headers: &HeaderMap,
    value: &str,
) -> Result<HostAuthority, ProxyMetadataError> {
    let parts = HostAuthority::authority_parts(value)
        .map_err(|_| ProxyMetadataError::InvalidForwardedHost)?;
    if parts.port().is_some() {
        return HostAuthority::retain_authority(value)
            .map_err(|_| ProxyMetadataError::InvalidForwardedHost);
    }

    let Some(port) = forwarded_port_from_headers(headers)? else {
        return HostAuthority::retain_authority(value)
            .map_err(|_| ProxyMetadataError::InvalidForwardedHost);
    };

    HostAuthority::new(format_authority_with_port(parts.host(), port))
        .map_err(|_| ProxyMetadataError::InvalidForwardedHost)
}

fn forwarded_proto_from_forwarded_header(
    headers: &HeaderMap,
) -> Result<Option<ForwardedProto>, ProxyMetadataError> {
    if let Some(value) = headers.get(FORWARDED_HEADER)
        && let Some(proto) = forwarded_header_parameter(value, "proto")
    {
        return parse_forwarded_proto_token(proto).map(Some);
    }

    Ok(None)
}

fn forwarded_proto_from_x_forwarded_headers(
    headers: &HeaderMap,
) -> Result<Option<ForwardedProto>, ProxyMetadataError> {
    if let Some(value) = headers.get(X_FORWARDED_PROTO_HEADER) {
        let proto = first_comma_separated_token(value)
            .ok_or(ProxyMetadataError::InvalidForwardedProto)
            .and_then(parse_forwarded_proto_token)?;
        return Ok(Some(proto));
    }

    Ok(None)
}

fn resolve_forwarded_proto_precedence(
    forwarded: Result<Option<ForwardedProto>, ProxyMetadataError>,
    x_forwarded: Result<Option<ForwardedProto>, ProxyMetadataError>,
    strict_forwarded_header_consistency: bool,
    listener_name: &str,
    route_template: &str,
) -> Result<Option<ForwardedProto>, ProxyMetadataError> {
    match (forwarded, x_forwarded) {
        (Ok(Some(forwarded_proto)), Ok(Some(x_forwarded_proto))) => {
            if forwarded_proto != x_forwarded_proto {
                if strict_forwarded_header_consistency {
                    return Err(ProxyMetadataError::ConflictingForwardedProto);
                }

                debug_forwarded_metadata_reason(
                    ForwardedMetadataDebugReason::ConflictingForwardedProto,
                    listener_name,
                    route_template,
                );
            }

            Ok(Some(forwarded_proto))
        }
        (Ok(Some(forwarded_proto)), Err(_)) => {
            debug_forwarded_metadata_reason(
                ForwardedMetadataDebugReason::InvalidFallbackForwardedProto,
                listener_name,
                route_template,
            );
            Ok(Some(forwarded_proto))
        }
        (Ok(Some(forwarded_proto)), Ok(None)) => Ok(Some(forwarded_proto)),
        (Err(error), _) => Err(error),
        (Ok(None), Ok(Some(x_forwarded_proto))) => Ok(Some(x_forwarded_proto)),
        (Ok(None), Err(error)) => Err(error),
        (Ok(None), Ok(None)) => Ok(None),
    }
}

fn parse_forwarded_proto_token(value: &str) -> Result<ForwardedProto, ProxyMetadataError> {
    ForwardedProto::parse(value).ok_or(ProxyMetadataError::InvalidForwardedProto)
}

fn forwarded_port_from_headers(
    headers: &HeaderMap,
) -> Result<Option<NetworkPort>, ProxyMetadataError> {
    let Some(value) = headers.get(X_FORWARDED_PORT_HEADER) else {
        return Ok(None);
    };
    let token =
        first_comma_separated_token(value).ok_or(ProxyMetadataError::InvalidForwardedHost)?;
    let port = token
        .parse::<u16>()
        .map_err(|_| ProxyMetadataError::InvalidForwardedHost)?;

    NetworkPort::new(port)
        .map(Some)
        .map_err(|_| ProxyMetadataError::InvalidForwardedHost)
}

fn forwarded_client_ip_from_headers(headers: &HeaderMap) -> Option<ForwardedClientIp> {
    headers
        .get(FORWARDED_HEADER)
        .and_then(forwarded_header_for_ip)
        .or_else(|| {
            headers
                .get(X_FORWARDED_FOR_HEADER)
                .and_then(first_forwarded_for_ip)
        })
        .or_else(|| headers.get(X_REAL_IP_HEADER).and_then(header_value_ip))
        .map(ForwardedClientIp)
}

fn direct_request_without_https_proof_allowed(
    request: &Request<Body>,
    peer_ip: Option<IpAddr>,
) -> bool {
    is_operational_route(request.uri().path()) && peer_ip.is_some_and(ip_is_loopback_or_private)
}

fn ip_is_loopback_or_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(address) => {
            address.is_loopback() || address.is_private() || address.is_link_local()
        }
        IpAddr::V6(address) => {
            address.is_loopback() || address.is_unique_local() || address.is_unicast_link_local()
        }
    }
}

fn debug_forwarded_metadata_reason(
    reason: ForwardedMetadataDebugReason,
    listener_name: &str,
    route_template: &str,
) {
    debug!(
        listener_name,
        route_template,
        reason = reason.as_str(),
        "trusted_proxy_metadata_normalized_by_precedence"
    );
}

fn first_comma_separated_token(value: &HeaderValue) -> Option<&str> {
    let value = value.to_str().ok()?;
    let first = value.split(',').next()?.trim();
    if first.is_empty() {
        return None;
    }

    Some(first)
}

fn first_forwarded_for_ip(value: &HeaderValue) -> Option<IpAddr> {
    let first = first_comma_separated_token(value)?;

    first.parse::<IpAddr>().ok()
}

fn header_value_ip(value: &HeaderValue) -> Option<IpAddr> {
    value.to_str().ok()?.trim().parse::<IpAddr>().ok()
}

fn forwarded_header_for_ip(value: &HeaderValue) -> Option<IpAddr> {
    let first_for = forwarded_header_parameter(value, "for")?;
    let normalized = first_for
        .trim_matches('"')
        .trim_start_matches('[')
        .trim_end_matches(']');

    normalized.parse::<IpAddr>().ok()
}

fn forwarded_header_parameter<'a>(value: &'a HeaderValue, key: &str) -> Option<&'a str> {
    let value = value.to_str().ok()?;
    let first_forwarded = value.split(',').next()?.trim();
    first_forwarded.split(';').find_map(|segment| {
        let (segment_key, segment_value) = segment.trim().split_once('=')?;
        if !segment_key.trim().eq_ignore_ascii_case(key) {
            return None;
        }

        let normalized = segment_value.trim().trim_matches('"');
        if normalized.is_empty() {
            return None;
        }

        Some(normalized)
    })
}

fn format_authority_with_port(host: &str, port: NetworkPort) -> String {
    if host.contains(':') {
        format!("[{host}]:{}", port.as_u16())
    } else {
        format!("{host}:{}", port.as_u16())
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
mod tests {
    use std::convert::Infallible;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
    use std::task::{Context, Poll};

    use axum::body::Body;
    use axum::extract::ConnectInfo;
    use axum::http::{Request, StatusCode};
    use axum::response::Response;
    use tower::{Layer, Service};

    use super::{
        CACHE_CONTROL_HEADER, CONTENT_SECURITY_POLICY_HEADER, ExternalRequestOrigin,
        FORWARDED_HEADER, ForwardedHost, ForwardedProto, REFERRER_POLICY_HEADER,
        X_CONTENT_TYPE_OPTIONS_HEADER, X_FORWARDED_FOR_HEADER, X_FORWARDED_HOST_HEADER,
        X_FORWARDED_PORT_HEADER, X_FORWARDED_PROTO_HEADER, X_FRAME_OPTIONS_HEADER,
        X_REAL_IP_HEADER, security_layer,
    };
    use crate::config::{
        HostAuthority, HostAuthorityPolicy, HttpHeaderBytesLimit, HttpHeaderCountLimit,
        HttpHeaderLimitConfig, HttpSecurityConfig, OperationalRouteAccess, SecurityHeadersConfig,
        TrustedProxyHeaders, TrustedProxyRange, TrustedProxyRequestMetadataConfig,
    };

    #[derive(Clone)]
    struct EchoHeadersService;

    impl Service<Request<Body>> for EchoHeadersService {
        type Response = Response;
        type Error = Infallible;
        type Future = std::future::Ready<Result<Response, Infallible>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: Request<Body>) -> Self::Future {
            let forwarded_present = request.headers().contains_key(FORWARDED_HEADER);
            let x_forwarded_for_present = request.headers().contains_key(X_FORWARDED_FOR_HEADER);
            let x_forwarded_host_present = request.headers().contains_key(X_FORWARDED_HOST_HEADER);
            let x_forwarded_port_present = request.headers().contains_key(X_FORWARDED_PORT_HEADER);
            let x_forwarded_proto_present =
                request.headers().contains_key(X_FORWARDED_PROTO_HEADER);
            let status = if forwarded_present
                || x_forwarded_for_present
                || x_forwarded_host_present
                || x_forwarded_port_present
                || x_forwarded_proto_present
            {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::NO_CONTENT
            };

            std::future::ready(Ok(Response::builder()
                .status(status)
                .body(Body::empty())
                .expect("test response should build")))
        }
    }

    #[derive(Clone)]
    struct ForwardedMetadataService;

    impl Service<Request<Body>> for ForwardedMetadataService {
        type Response = Response;
        type Error = Infallible;
        type Future = std::future::Ready<Result<Response, Infallible>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: Request<Body>) -> Self::Future {
            let raw_proxy_header_present = request.headers().contains_key(FORWARDED_HEADER)
                || request.headers().contains_key(X_FORWARDED_FOR_HEADER)
                || request.headers().contains_key(X_REAL_IP_HEADER)
                || request.headers().contains_key(X_FORWARDED_HOST_HEADER)
                || request.headers().contains_key(X_FORWARDED_PORT_HEADER)
                || request.headers().contains_key(X_FORWARDED_PROTO_HEADER);
            let client_ip = request
                .extensions()
                .get::<super::ForwardedClientIp>()
                .map(|client_ip| (*client_ip).into_ip_addr());
            let host = request
                .extensions()
                .get::<ForwardedHost>()
                .map(|host| host.authority().as_str().to_owned());
            let proto = request.extensions().get::<ForwardedProto>().copied();
            let origin = request
                .extensions()
                .get::<ExternalRequestOrigin>()
                .map(ExternalRequestOrigin::base_url);
            let status = match (raw_proxy_header_present, client_ip, host, proto, origin) {
                (
                    false,
                    Some(IpAddr::V4(ip)),
                    Some(host),
                    Some(ForwardedProto::Https),
                    Some(origin),
                ) if ip == Ipv4Addr::new(203, 0, 113, 7)
                    && host == "api.reallyme.net:443"
                    && origin == "https://api.reallyme.net:443" =>
                {
                    StatusCode::ACCEPTED
                }
                _ => StatusCode::BAD_REQUEST,
            };

            std::future::ready(Ok(Response::builder()
                .status(status)
                .body(Body::empty())
                .expect("test response should build")))
        }
    }

    fn secure_config_with_host_policy(policy: HostAuthorityPolicy) -> HttpSecurityConfig {
        HttpSecurityConfig::new(
            SecurityHeadersConfig::secure_defaults(),
            policy,
            TrustedProxyHeaders::ignore_all(),
            TrustedProxyRequestMetadataConfig::secure_defaults(),
            OperationalRouteAccess::Public,
        )
    }

    fn trusted_proxy_metadata_config(allowed_hosts: Vec<HostAuthority>) -> HttpSecurityConfig {
        trusted_proxy_metadata_config_with_strict_mode(allowed_hosts, false)
    }

    fn trusted_proxy_metadata_config_with_strict_mode(
        allowed_hosts: Vec<HostAuthority>,
        strict_forwarded_header_consistency: bool,
    ) -> HttpSecurityConfig {
        HttpSecurityConfig::new(
            SecurityHeadersConfig::secure_defaults(),
            HostAuthorityPolicy::allow_list(allowed_hosts).expect("non-empty allowlist"),
            TrustedProxyHeaders::trust_configured_proxies(vec![
                TrustedProxyRange::parse("10.0.0.0/8").expect("valid proxy range"),
            ])
            .expect("non-empty proxy ranges"),
            TrustedProxyRequestMetadataConfig::new(
                true,
                true,
                true,
                strict_forwarded_header_consistency,
                true,
            ),
            OperationalRouteAccess::Public,
        )
    }

    #[tokio::test]
    async fn security_headers_are_added_without_overwriting_existing_values() {
        let config = HttpSecurityConfig::secure_defaults();
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let request = Request::builder()
            .uri("/app")
            .header("host", "api.reallyme.net")
            .body(Body::empty())
            .expect("test request should build");

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(
            response.headers().get(X_CONTENT_TYPE_OPTIONS_HEADER),
            Some(&axum::http::HeaderValue::from_static("nosniff"))
        );
        assert!(response.headers().contains_key(REFERRER_POLICY_HEADER));
        assert!(response.headers().contains_key(X_FRAME_OPTIONS_HEADER));
        assert!(
            response
                .headers()
                .contains_key(CONTENT_SECURITY_POLICY_HEADER)
        );
    }

    #[tokio::test]
    async fn disallowed_host_authority_returns_stable_bad_request() {
        let policy = HostAuthorityPolicy::allow_list(vec![
            HostAuthority::new("api.reallyme.net").expect("valid host"),
        ])
        .expect("non-empty allowlist");
        let config = secure_config_with_host_policy(policy);
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let request = Request::builder()
            .uri("/app")
            .header("host", "evil.example")
            .body(Body::empty())
            .expect("test request should build");

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers().get(CACHE_CONTROL_HEADER),
            Some(&axum::http::HeaderValue::from_static("no-store"))
        );
    }

    #[tokio::test]
    async fn host_authority_missing_is_rejected_with_allowlist_policy() {
        let policy = HostAuthorityPolicy::allow_list(vec![
            HostAuthority::new("api.reallyme.net").expect("valid host"),
        ])
        .expect("non-empty allowlist");
        let config = secure_config_with_host_policy(policy);
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let request = Request::builder()
            .uri("/app")
            .body(Body::empty())
            .expect("test request should build");

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers().get(CACHE_CONTROL_HEADER),
            Some(&axum::http::HeaderValue::from_static("no-store"))
        );
    }

    #[tokio::test]
    async fn untrusted_proxy_headers_are_stripped_before_handlers() {
        let config = HttpSecurityConfig::secure_defaults();
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let request = Request::builder()
            .uri("/app")
            .header("host", "api.reallyme.net")
            .header("forwarded", "for=203.0.113.1")
            .header("x-forwarded-for", "203.0.113.1")
            .body(Body::empty())
            .expect("test request should build");

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn trusted_proxy_ranges_allow_normalized_forwarded_metadata_without_raw_headers() {
        let config = trusted_proxy_metadata_config(vec![
            HostAuthority::new("api.reallyme.net:443").expect("valid host"),
        ]);
        let mut service = security_layer(&config).layer(ForwardedMetadataService);
        let trusted = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 1, 2, 3), 40_000));
        let mut request = Request::builder()
            .uri("/app")
            .header("host", "internal-lb.local")
            .header(
                "forwarded",
                "for=203.0.113.7;host=api.reallyme.net:443;proto=https",
            )
            .header("x-forwarded-for", "198.51.100.9, 198.51.100.1")
            .header("x-forwarded-host", "api.reallyme.net")
            .header("x-forwarded-port", "443")
            .header("x-forwarded-proto", "https")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(trusted));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn trusted_proxy_prefers_forwarded_over_x_forwarded_when_not_strict() {
        let config = trusted_proxy_metadata_config(vec![
            HostAuthority::new("api.reallyme.net").expect("valid host"),
        ]);
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let trusted = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 1, 2, 3), 40_000));
        let mut request = Request::builder()
            .uri("/app")
            .header("host", "internal-lb.local")
            .header(
                "forwarded",
                "for=203.0.113.7;host=api.reallyme.net;proto=https",
            )
            .header("x-forwarded-host", "evil.example")
            .header("x-forwarded-proto", "http")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(trusted));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn strict_mode_rejects_conflicting_forwarded_host_and_proto() {
        let config = trusted_proxy_metadata_config_with_strict_mode(
            vec![HostAuthority::new("api.reallyme.net").expect("valid host")],
            true,
        );
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let trusted = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 1, 2, 3), 40_000));
        let mut request = Request::builder()
            .uri("/app")
            .header("host", "internal-lb.local")
            .header(
                "forwarded",
                "for=203.0.113.7;host=api.reallyme.net;proto=https",
            )
            .header("x-forwarded-host", "evil.example")
            .header("x-forwarded-proto", "http")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(trusted));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn trusted_proxy_forwarded_host_is_accepted_when_allowlisted() {
        let config = trusted_proxy_metadata_config(vec![
            HostAuthority::new("api.reallyme.net").expect("valid host"),
        ]);
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let trusted = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 1, 2, 3), 40_000));
        let mut request = Request::builder()
            .uri("/app")
            .header("host", "internal-lb.local")
            .header("x-forwarded-host", "api.reallyme.net")
            .header("x-forwarded-proto", "https")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(trusted));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn trusted_proxy_forwarded_host_is_rejected_when_not_allowlisted() {
        let config = trusted_proxy_metadata_config(vec![
            HostAuthority::new("reallyme.net").expect("valid host"),
        ]);
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let trusted = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 1, 2, 3), 40_000));
        let mut request = Request::builder()
            .uri("/app")
            .header("host", "internal-lb.local")
            .header("x-forwarded-host", "api.reallyme.net")
            .header("x-forwarded-proto", "https")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(trusted));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn untrusted_peer_spoofed_forwarded_host_is_ignored_and_direct_host_is_used() {
        let config = secure_config_with_host_policy(
            HostAuthorityPolicy::allow_list(vec![
                HostAuthority::new("api.reallyme.net").expect("valid host"),
            ])
            .expect("non-empty allowlist"),
        );
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let remote = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 10), 40_000));
        let mut request = Request::builder()
            .uri("/app")
            .header("host", "api.reallyme.net")
            .header("x-forwarded-host", "evil.example")
            .header("x-forwarded-proto", "https")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(remote));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn trusted_proxy_forwarded_proto_https_is_accepted() {
        let config = trusted_proxy_metadata_config(vec![
            HostAuthority::new("api.reallyme.net").expect("valid host"),
        ]);
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let trusted = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 1, 2, 3), 40_000));
        let mut request = Request::builder()
            .uri("/app")
            .header("host", "internal-lb.local")
            .header("x-forwarded-host", "api.reallyme.net")
            .header("x-forwarded-proto", "https")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(trusted));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn untrusted_peer_spoofed_forwarded_proto_is_ignored() {
        let config = secure_config_with_host_policy(
            HostAuthorityPolicy::allow_list(vec![
                HostAuthority::new("api.reallyme.net").expect("valid host"),
            ])
            .expect("non-empty allowlist"),
        );
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let remote = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 10), 40_000));
        let mut request = Request::builder()
            .uri("/app")
            .header("host", "api.reallyme.net")
            .header("x-forwarded-proto", "https")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(remote));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn direct_operational_health_check_is_allowed_without_https_proof_when_private() {
        let config = HttpSecurityConfig::new(
            SecurityHeadersConfig::secure_defaults(),
            HostAuthorityPolicy::allow_list(vec![
                HostAuthority::new("api.reallyme.net").expect("valid host"),
            ])
            .expect("non-empty allowlist"),
            TrustedProxyHeaders::ignore_all(),
            TrustedProxyRequestMetadataConfig::new(false, false, true, false, true),
            OperationalRouteAccess::LocalOnly,
        );
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let loopback = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 40_000));
        let mut request = Request::builder()
            .uri("/healthz")
            .header("host", "api.reallyme.net")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(loopback));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn direct_non_operational_request_is_rejected_without_https_proof_when_required() {
        let config = HttpSecurityConfig::new(
            SecurityHeadersConfig::secure_defaults(),
            HostAuthorityPolicy::allow_list(vec![
                HostAuthority::new("api.reallyme.net").expect("valid host"),
            ])
            .expect("non-empty allowlist"),
            TrustedProxyHeaders::ignore_all(),
            TrustedProxyRequestMetadataConfig::new(false, false, true, false, true),
            OperationalRouteAccess::Public,
        );
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let loopback = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 40_000));
        let mut request = Request::builder()
            .uri("/app")
            .header("host", "api.reallyme.net")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(loopback));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn operational_routes_are_local_only_by_default_when_peer_is_known() {
        let config = HttpSecurityConfig::secure_defaults();
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let remote = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 10), 40_000));
        let mut request = Request::builder()
            .uri("/metrics")
            .header("host", "api.reallyme.net")
            .body(Body::empty())
            .expect("test request should build");
        request.extensions_mut().insert(ConnectInfo(remote));

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn operational_routes_are_local_only_by_default_when_peer_is_unknown() {
        let config = HttpSecurityConfig::secure_defaults();
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let request = Request::builder()
            .uri("/healthz")
            .header("host", "api.reallyme.net")
            .body(Body::empty())
            .expect("test request should build");

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn excessive_header_count_returns_stable_header_limit_error() {
        let header_limits = HttpHeaderLimitConfig::new(
            HttpHeaderCountLimit::new(1).expect("valid count limit"),
            HttpHeaderBytesLimit::new(4096).expect("valid byte limit"),
        );
        let config = HttpSecurityConfig::secure_defaults().with_header_limits(header_limits);
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let request = Request::builder()
            .uri("/app")
            .header("host", "api.reallyme.net")
            .header("x-extra", "value")
            .body(Body::empty())
            .expect("test request should build");

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(
            response.status(),
            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE
        );
    }

    #[tokio::test]
    async fn excessive_header_bytes_returns_stable_header_limit_error() {
        let header_limits = HttpHeaderLimitConfig::new(
            HttpHeaderCountLimit::new(16).expect("valid count limit"),
            HttpHeaderBytesLimit::new(1024).expect("valid byte limit"),
        );
        let config = HttpSecurityConfig::secure_defaults().with_header_limits(header_limits);
        let mut service = security_layer(&config).layer(EchoHeadersService);
        let request = Request::builder()
            .uri("/app")
            .header("host", "api.reallyme.net")
            .header("x-large", "x".repeat(2048))
            .body(Body::empty())
            .expect("test request should build");

        let response = service.call(request).await.expect("infallible service");

        assert_eq!(
            response.status(),
            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE
        );
    }
}

#[cfg(test)]
#[path = "security/header_limit_tests.rs"]
mod header_limit_tests;
