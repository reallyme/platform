// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    GrpcServePolicy, grpc_health_status_for_readiness, serve_health_grpc,
    set_registered_health_status, sync_grpc_health_with_readiness,
};
use crate::config::{
    ConnectionLimitConfig, DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT, TrustedProxyHeaders,
};
use crate::grpc::{GrpcHealthServingStatus, health_reporter};
use crate::health::Readiness;
use crate::runtime::RateLimitRegistry;
use crate::shutdown::ShutdownReason;
use crate::task::ShutdownController;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tonic::server::NamedService;
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_client::HealthClient;
use tonic_health::pb::health_server::Health;

#[derive(Clone)]
struct PendingGrpcService {
    started: Arc<tokio::sync::Notify>,
    dropped: Arc<AtomicBool>,
}

impl tonic::server::NamedService for PendingGrpcService {
    const NAME: &'static str = "test.v1.PendingService";
}

struct PendingGrpcResponse {
    started: Arc<tokio::sync::Notify>,
    dropped: Arc<AtomicBool>,
}

impl std::future::Future for PendingGrpcResponse {
    type Output =
        Result<tonic::codegen::http::Response<tonic::body::Body>, std::convert::Infallible>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.started.notify_one();
        std::task::Poll::Pending
    }
}

impl Drop for PendingGrpcResponse {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

impl tower::Service<tonic::codegen::http::Request<tonic::body::Body>> for PendingGrpcService {
    type Response = tonic::codegen::http::Response<tonic::body::Body>;
    type Error = std::convert::Infallible;
    type Future = PendingGrpcResponse;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _request: tonic::codegen::http::Request<tonic::body::Body>) -> Self::Future {
        PendingGrpcResponse {
            started: Arc::clone(&self.started),
            dropped: Arc::clone(&self.dropped),
        }
    }
}

async fn health_status(reporter: &crate::grpc::GrpcHealthReporter, service_name: &str) -> i32 {
    let service = tonic_health::server::HealthService::from_health_reporter(reporter.clone());
    service
        .check(tonic::Request::new(HealthCheckRequest {
            service: service_name.to_owned(),
        }))
        .await
        .expect("overall health service should be registered")
        .into_inner()
        .status
}

#[test]
fn grpc_health_tracks_current_readiness_state() {
    let readiness = Readiness::new();

    assert_eq!(
        grpc_health_status_for_readiness(&readiness),
        GrpcHealthServingStatus::NotServing
    );

    readiness.mark_ready();

    assert_eq!(
        grpc_health_status_for_readiness(&readiness),
        GrpcHealthServingStatus::Serving
    );

    readiness.mark_not_ready();

    assert_eq!(
        grpc_health_status_for_readiness(&readiness),
        GrpcHealthServingStatus::NotServing
    );
}

#[tokio::test]
async fn grpc_health_updates_with_readiness_and_shutdown() {
    const APP_SERVICE: &str = "reallyme.example.v1.ExampleService";
    let readiness = Readiness::new();
    let watcher = readiness.watch();
    let (mut reporter, _health_service) = health_reporter();
    set_registered_health_status(&mut reporter, &readiness, &[APP_SERVICE]).await;
    let controller = ShutdownController::new();
    let sync = tokio::spawn(sync_grpc_health_with_readiness(
        reporter.clone(),
        readiness.clone(),
        watcher,
        vec![APP_SERVICE],
        controller.token(),
    ));

    readiness.mark_ready();
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while health_status(&reporter, APP_SERVICE).await
            != tonic_health::pb::health_check_response::ServingStatus::Serving as i32
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("health should become serving");

    readiness.mark_not_ready();
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while health_status(&reporter, APP_SERVICE).await
            != tonic_health::pb::health_check_response::ServingStatus::NotServing as i32
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("health should become not serving");

    controller.begin_shutdown(ShutdownReason::Sigterm);
    sync.await.expect("health sync should stop after shutdown");
    assert_eq!(
        health_status(&reporter, APP_SERVICE).await,
        tonic_health::pb::health_check_response::ServingStatus::NotServing as i32
    );
}

#[tokio::test]
async fn aborting_grpc_serve_closes_detached_watch_connections() {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind local gRPC fixture");
    let address = listener.local_addr().expect("local gRPC fixture address");
    let controller = ShutdownController::new();
    let rate_limit_registry = Arc::new(RateLimitRegistry::new(Arc::new(Vec::new())));
    let policy = GrpcServePolicy {
        listener_name: Arc::<str>::from("test-grpc"),
        max_concurrent_streams: None,
        deadline_required: false,
        max_timeout: None,
        max_header_list_size_bytes: 64 * 1024,
        method_policies: Vec::new(),
        rate_limit_registry,
        trusted_proxy_headers: TrustedProxyHeaders::ignore_all(),
        connection_limits: ConnectionLimitConfig::secure_defaults(),
    };
    let serve = tokio::spawn(serve_health_grpc(
        listener,
        None,
        Vec::new(),
        Readiness::new(),
        policy,
        DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT,
        controller.token(),
    ));

    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
        .expect("valid local gRPC endpoint")
        .connect()
        .await
        .expect("connect local gRPC fixture");
    let mut client = HealthClient::new(channel);
    let mut watch = client
        .watch(tonic::Request::new(HealthCheckRequest {
            service: String::new(),
        }))
        .await
        .expect("health watch should start")
        .into_inner();
    watch
        .message()
        .await
        .expect("initial health status should decode")
        .expect("initial health status should exist");

    serve.abort();
    let aborted = serve.await.expect_err("serve future should be aborted");
    assert!(aborted.is_cancelled());
    let result = tokio::time::timeout(Duration::from_secs(1), watch.message())
        .await
        .expect("detached watch connection should close");
    assert!(result.is_err() || result.expect("stream result").is_none());
}

#[tokio::test]
async fn routed_grpc_watch_survives_first_request_deadline() {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind local gRPC fixture");
    let address = listener.local_addr().expect("local gRPC fixture address");
    let controller = ShutdownController::new();
    let policy = GrpcServePolicy {
        listener_name: Arc::<str>::from("test-grpc"),
        max_concurrent_streams: None,
        deadline_required: false,
        max_timeout: None,
        max_header_list_size_bytes: 64 * 1024,
        method_policies: Vec::new(),
        rate_limit_registry: Arc::new(RateLimitRegistry::new(Arc::new(Vec::new()))),
        trusted_proxy_headers: TrustedProxyHeaders::ignore_all(),
        connection_limits: ConnectionLimitConfig::secure_defaults(),
    };
    let serve = tokio::spawn(serve_health_grpc(
        listener,
        None,
        Vec::new(),
        Readiness::new(),
        policy,
        DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT,
        controller.token(),
    ));
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
        .expect("valid local gRPC endpoint")
        .connect()
        .await
        .expect("connect local gRPC fixture");
    let mut watch = HealthClient::new(channel)
        .watch(tonic::Request::new(HealthCheckRequest {
            service: String::new(),
        }))
        .await
        .expect("health watch should start")
        .into_inner();
    assert!(watch.message().await.expect("initial status").is_some());

    tokio::time::sleep(Duration::from_secs(6)).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(200), watch.message())
            .await
            .is_err(),
        "a routed request must release the first-request deadline"
    );

    serve.abort();
    assert!(
        serve
            .await
            .expect_err("serve future should abort")
            .is_cancelled()
    );
}

#[tokio::test]
async fn aborting_grpc_serve_cancels_in_flight_app_handler() {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind local gRPC fixture");
    let address = listener.local_addr().expect("local gRPC fixture address");
    let started = Arc::new(tokio::sync::Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let app_service = PendingGrpcService {
        started: Arc::clone(&started),
        dropped: Arc::clone(&dropped),
    };
    let policy = GrpcServePolicy {
        listener_name: Arc::<str>::from("test-grpc"),
        max_concurrent_streams: None,
        deadline_required: false,
        max_timeout: None,
        max_header_list_size_bytes: 64 * 1024,
        method_policies: Vec::new(),
        rate_limit_registry: Arc::new(RateLimitRegistry::new(Arc::new(Vec::new()))),
        trusted_proxy_headers: TrustedProxyHeaders::ignore_all(),
        connection_limits: ConnectionLimitConfig::secure_defaults(),
    };
    let controller = ShutdownController::new();
    let serve = tokio::spawn(serve_health_grpc(
        listener,
        Some(tonic::service::Routes::new(app_service)),
        vec![PendingGrpcService::NAME],
        Readiness::new(),
        policy,
        DEFAULT_GRPC_IN_FLIGHT_REQUEST_LIMIT,
        controller.token(),
    ));
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}"))
        .expect("valid local gRPC endpoint")
        .connect()
        .await
        .expect("connect local gRPC fixture");
    let call = tokio::spawn(async move {
        let mut client = tonic::client::Grpc::new(channel);
        client.ready().await.expect("client should become ready");
        client
            .unary(
                tonic::Request::new(HealthCheckRequest {
                    service: String::new(),
                }),
                tonic::codegen::http::uri::PathAndQuery::from_static(
                    "/test.v1.PendingService/Call",
                ),
                tonic_prost::ProstCodec::<
                    HealthCheckRequest,
                    tonic_health::pb::HealthCheckResponse,
                >::default(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), started.notified())
        .await
        .expect("app handler should start");

    serve.abort();
    assert!(
        serve
            .await
            .expect_err("serve future should abort")
            .is_cancelled()
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        while !dropped.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("in-flight handler should be cancelled before cleanup");
    call.abort();
}
