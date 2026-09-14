// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::Infallible;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::task::{Context, Poll};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use tower::{Layer, Service};

use super::{
    CACHE_CONTROL_HEADER, CONTENT_SECURITY_POLICY_HEADER, ExternalRequestOrigin, FORWARDED_HEADER,
    ForwardedHost, ForwardedProto, REFERRER_POLICY_HEADER, X_CONTENT_TYPE_OPTIONS_HEADER,
    X_FORWARDED_FOR_HEADER, X_FORWARDED_HOST_HEADER, X_FORWARDED_PORT_HEADER,
    X_FORWARDED_PROTO_HEADER, X_FRAME_OPTIONS_HEADER, X_REAL_IP_HEADER, security_layer,
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
        let x_forwarded_proto_present = request.headers().contains_key(X_FORWARDED_PROTO_HEADER);
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
