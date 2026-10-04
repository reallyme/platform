// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::runtime::RateLimitRegistry;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tonic::body::Body as TonicBody;
use tonic::codegen::http::{Request, Response};
use tower::{Layer, Service, ServiceExt, service_fn};

use super::{GrpcPolicy, grpc_policy_layer};
use crate::config::{TrustedProxyHeaders, TrustedProxyRange};
use crate::runtime::GrpcMethodPolicy;
use crate::runtime::RateLimitSourceIdentity;

#[test]
fn grpc_source_identity_uses_tonic_tcp_peer_metadata() {
    let mut request = Request::new(TonicBody::empty());
    let peer: std::net::SocketAddr = "[::ffff:192.0.2.5]:34567"
        .parse()
        .expect("valid mapped peer fixture");
    request
        .extensions_mut()
        .insert(tonic::transport::server::TcpConnectInfo {
            local_addr: None,
            remote_addr: Some(peer),
        });

    assert_eq!(super::request_peer_ip(&request), Some(peer.ip()));
}

#[test]
fn grpc_trusted_proxy_uses_nearest_untrusted_x_forwarded_for_hop() {
    let mut request = Request::new(TonicBody::empty());
    request
        .extensions_mut()
        .insert(tonic::transport::server::TcpConnectInfo {
            local_addr: None,
            remote_addr: Some("10.1.2.3:4222".parse().expect("trusted peer fixture")),
        });
    request.headers_mut().insert(
        "x-forwarded-for",
        "6.6.6.6, 203.0.113.9"
            .parse()
            .expect("valid header fixture"),
    );
    let trusted = TrustedProxyHeaders::trust_configured_proxies(vec![
        TrustedProxyRange::parse("10.0.0.0/8").expect("valid range fixture"),
    ])
    .expect("nonempty trusted range fixture");

    assert_eq!(
        super::request_source_identity(&request, &trusted),
        RateLimitSourceIdentity::ForwardedIp(
            "203.0.113.9".parse().expect("valid client IP fixture")
        )
    );

    let untrusted = TrustedProxyHeaders::ignore_all();
    assert!(matches!(
        super::request_source_identity(&request, &untrusted),
        RateLimitSourceIdentity::PeerIp(_)
    ));
}

#[tokio::test]
async fn malformed_grpc_deadlines_are_rejected_before_handler() {
    let called = Arc::new(AtomicBool::new(false));
    let called_inner = Arc::clone(&called);
    let inner = service_fn(move |_: Request<TonicBody>| {
        called_inner.store(true, Ordering::SeqCst);
        async move { Ok::<Response<TonicBody>, Infallible>(Response::new(TonicBody::empty())) }
    });
    let policy = GrpcPolicy::new(
        Arc::<str>::from("grpc-public"),
        None,
        true,
        Vec::new(),
        Arc::new(RateLimitRegistry::new(Arc::new(Vec::new()))),
    );
    let mut service = grpc_policy_layer(policy).layer(inner);

    for malformed in ["+1S", " 2S", "abc", "9999999999S"] {
        let request = Request::builder()
            .uri("/reallyme.api.v1.ApiStatusService/GetStatus")
            .header("grpc-timeout", malformed)
            .body(TonicBody::empty())
            .expect("valid grpc request fixture");
        let response = service
            .ready()
            .await
            .expect("service should be ready")
            .call(request)
            .await
            .expect("policy response should succeed");
        assert_eq!(response.headers().get("grpc-status").expect("status"), "3");
    }
    assert!(!called.load(Ordering::SeqCst));
}

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
