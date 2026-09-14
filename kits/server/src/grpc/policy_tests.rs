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
