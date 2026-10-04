// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::serve::Listener;
use futures_util::stream;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tonic::service::Routes;
use tonic::transport::Server;
use tonic_health::pb::health_server::HealthServer;
use tower::ServiceBuilder;
use tower::limit::ConcurrencyLimitLayer;
use tower::load_shed::LoadShedLayer;

use super::connection_guard::{BoundedTcpListener, ForceCloseConnections};
use super::grpc_idle::GrpcActivityLayer;
use crate::config::{
    ConnectionLimitConfig, GrpcServerConfig, RuntimeConcurrencyLimit, TrustedProxyHeaders,
};
use crate::grpc::{GrpcHealthServingStatus, health_reporter};
use crate::grpc::{GrpcTimeout, grpc_policy_layer};
use crate::health::{GrpcServingStatus, Readiness, ReadinessWatcher, readiness_check};
use crate::http::HttpRateLimitTierName;
use crate::runtime::{HttpRateLimitTierPolicy, RateLimitRegistry};
use crate::startup::TaskName;
use crate::task::{ShutdownToken, TaskExecutionError, TaskExecutionErrorKind};

const DEFAULT_GRPC_HTTP2_MAX_HEADER_LIST_SIZE: u32 = 64 * 1024;
const DEFAULT_GRPC_MAX_CONCURRENT_STREAMS: u32 = 128;
const GRPC_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);
const GRPC_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(10);
// Tonic emits GOAWAY at max age. It has no idle GOAWAY hook, so retire
// connections gracefully before the IO fallback can close an idle socket.
// Keep a long grace for active streams after GOAWAY; the short age must not
// turn a healthy server stream into an early transport failure.
const GRPC_CONNECTION_MAX_AGE: Duration = Duration::from_secs(60);
const GRPC_CONNECTION_MAX_AGE_GRACE: Duration = Duration::from_secs(600);

struct CloseConnectionsOnDrop(Arc<ForceCloseConnections>);

impl Drop for CloseConnectionsOnDrop {
    fn drop(&mut self) {
        // Tonic owns detached per-connection tasks. If the supervisor aborts
        // this serving future at the drain deadline, wake their IO so those
        // tasks cannot keep serving while app cleanup releases dependencies.
        self.0.close();
    }
}

#[path = "grpc/app_routes.rs"]
mod app_routes;
pub use app_routes::{GrpcAppRoutes, GrpcAppRoutesError, GrpcAppRoutesErrorReason};
#[path = "grpc/health_service.rs"]
mod health_service;
use health_service::RuntimeHealthService;

/// gRPC server input owned by server composition and run by [`crate::runtime::ServerRuntime`].
///
/// The spec supports both health-only servers and app-provided tonic
/// routes. In both cases the runtime mounts standard gRPC health and owns the
/// listener/serve/shutdown mechanics.
///
/// App-owned tonic services must configure their own decode and encode limits.
/// Per-method `max_request_message_bytes` is an early header check, not full
/// streaming byte accounting.
pub struct GrpcServerSpec {
    task_name: TaskName,
    config: GrpcServerConfig,
    routes: Option<GrpcAppRoutes>,
    max_concurrent_streams: Option<u32>,
    deadline_required: bool,
    max_timeout: Option<GrpcTimeout>,
    max_header_list_size_bytes: u32,
    method_policies: Vec<GrpcMethodPolicy>,
    rate_limit_policies: std::sync::Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>>,
    trusted_proxy_headers: TrustedProxyHeaders,
}

impl GrpcServerSpec {
    /// Creates a standard health-only gRPC server spec.
    pub fn health_only(task_name: TaskName, config: GrpcServerConfig) -> Self {
        Self {
            task_name,
            config,
            routes: None,
            max_concurrent_streams: None,
            deadline_required: false,
            max_timeout: None,
            max_header_list_size_bytes: DEFAULT_GRPC_HTTP2_MAX_HEADER_LIST_SIZE,
            method_policies: Vec::new(),
            rate_limit_policies: std::sync::Arc::new(Vec::new()),
            trusted_proxy_headers: TrustedProxyHeaders::ignore_all(),
        }
    }

    /// Creates a gRPC server spec from app-owned tonic routes.
    ///
    /// The runtime will mount the standard gRPC health service alongside these
    /// routes and publish readiness under each registered service name. Build
    /// routes through [`GrpcAppRoutes`] so named health cannot silently remain
    /// unknown. App crates provide product/internal RPC services only.
    pub fn with_routes(
        task_name: TaskName,
        config: GrpcServerConfig,
        routes: GrpcAppRoutes,
    ) -> Self {
        Self {
            task_name,
            config,
            routes: Some(routes),
            max_concurrent_streams: None,
            deadline_required: false,
            max_timeout: None,
            max_header_list_size_bytes: DEFAULT_GRPC_HTTP2_MAX_HEADER_LIST_SIZE,
            method_policies: Vec::new(),
            rate_limit_policies: std::sync::Arc::new(Vec::new()),
            trusted_proxy_headers: TrustedProxyHeaders::ignore_all(),
        }
    }
    /// Attaches optional HTTP/2 concurrent stream cap.
    pub fn with_max_concurrent_streams(mut self, max_concurrent_streams: Option<u32>) -> Self {
        self.max_concurrent_streams = max_concurrent_streams;
        self
    }

    /// Enforces caller deadline metadata requirements for native gRPC.
    pub fn with_deadline_policy(
        mut self,
        deadline_required: bool,
        max_timeout: Option<GrpcTimeout>,
    ) -> Self {
        self.deadline_required = deadline_required;
        self.max_timeout = max_timeout;
        self
    }

    /// Overrides HTTP/2 header-list-size cap for native gRPC listener.
    pub fn with_max_header_list_size_bytes(mut self, max_header_list_size_bytes: u32) -> Self {
        self.max_header_list_size_bytes = max_header_list_size_bytes;
        self
    }

    /// Attaches per-method native gRPC policy overrides.
    pub fn with_method_policies(mut self, method_policies: Vec<GrpcMethodPolicy>) -> Self {
        self.method_policies = method_policies;
        self
    }

    /// Attaches named rate-limit policies referenced by method policies.
    pub fn with_rate_limit_policies(
        mut self,
        rate_limit_policies: std::sync::Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>>,
    ) -> Self {
        self.rate_limit_policies = rate_limit_policies;
        self
    }

    /// Trusts X-Forwarded-For only from the configured gRPC ingress ranges.
    pub fn with_trusted_proxy_headers(mut self, headers: TrustedProxyHeaders) -> Self {
        self.trusted_proxy_headers = headers;
        self
    }

    pub(crate) fn trusted_proxy_headers(&self) -> TrustedProxyHeaders {
        self.trusted_proxy_headers.clone()
    }

    pub(crate) fn task_name(&self) -> TaskName {
        self.task_name.clone()
    }

    pub(crate) fn config(&self) -> GrpcServerConfig {
        self.config
    }

    pub(crate) fn into_routes(self) -> (Option<Routes>, Vec<&'static str>) {
        match self.routes {
            Some(routes) => {
                let (routes, names) = routes.into_parts();
                (Some(routes), names)
            }
            None => (None, Vec::new()),
        }
    }

    pub(crate) fn max_concurrent_streams(&self) -> Option<u32> {
        self.max_concurrent_streams
    }

    pub(crate) fn deadline_required(&self) -> bool {
        self.deadline_required
    }

    pub(crate) fn max_timeout(&self) -> Option<GrpcTimeout> {
        self.max_timeout
    }

    pub(crate) fn max_header_list_size_bytes(&self) -> u32 {
        self.max_header_list_size_bytes
    }

    pub(crate) fn method_policies(&self) -> Vec<GrpcMethodPolicy> {
        self.method_policies.clone()
    }

    pub(crate) fn rate_limit_policies(
        &self,
    ) -> std::sync::Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>> {
        std::sync::Arc::clone(&self.rate_limit_policies)
    }
}

pub(crate) async fn serve_health_grpc(
    listener: TcpListener,
    routes: Option<Routes>,
    service_names: Vec<&'static str>,
    readiness: Readiness,
    policy: GrpcServePolicy,
    concurrency_limit: RuntimeConcurrencyLimit,
    shutdown: ShutdownToken,
) -> Result<(), TaskExecutionError> {
    let force_close = Arc::new(ForceCloseConnections::new());
    let _close_connections_on_drop = CloseConnectionsOnDrop(Arc::clone(&force_close));
    let (mut reporter, _health_service) = health_reporter();
    let watcher = readiness.watch();
    set_registered_health_status(&mut reporter, &readiness, &service_names).await;
    let service_names_for_exit = service_names.clone();
    let health_sync = sync_grpc_health_with_readiness(
        reporter.clone(),
        readiness.clone(),
        watcher,
        service_names,
        shutdown.clone(),
    );
    let health_service = HealthServer::new(RuntimeHealthService::new(
        tonic_health::server::HealthService::from_health_reporter(reporter.clone()),
    ));
    let routes = match routes {
        Some(routes) => routes.add_service(health_service),
        None => Routes::new(health_service),
    };

    let mut shutdown_for_server = shutdown.clone();
    let trusted_proxies = policy.trusted_proxy_headers.clone();
    let grpc_policy = crate::grpc::GrpcPolicy::new(
        policy.listener_name,
        policy.max_timeout,
        policy.deadline_required,
        policy.method_policies,
        policy.rate_limit_registry,
    )
    .with_trusted_proxy_headers(policy.trusted_proxy_headers);
    let mut builder = Server::builder()
        .http2_max_header_list_size(policy.max_header_list_size_bytes)
        .max_concurrent_streams(
            policy
                .max_concurrent_streams
                .unwrap_or(DEFAULT_GRPC_MAX_CONCURRENT_STREAMS),
        )
        .http2_keepalive_interval(Some(GRPC_KEEPALIVE_INTERVAL))
        .http2_keepalive_timeout(Some(GRPC_KEEPALIVE_TIMEOUT))
        .max_connection_age(GRPC_CONNECTION_MAX_AGE)
        .max_connection_age_grace(GRPC_CONNECTION_MAX_AGE_GRACE);
    if let Some(max_timeout) = policy.max_timeout {
        builder = builder.timeout(max_timeout.as_duration());
    }
    let incoming = stream::unfold(
        BoundedTcpListener::with_force_close(listener, force_close, policy.connection_limits)
            .with_trusted_proxies(trusted_proxies),
        |mut listener| async move {
            let (stream, _peer) = listener.accept().await;
            Some((Ok::<_, std::io::Error>(stream), listener))
        },
    );
    let server = builder
        .layer(
            ServiceBuilder::new()
                .layer(GrpcActivityLayer)
                .layer(grpc_policy_layer(grpc_policy))
                // Load shedding keeps policy dispatch ready even when the
                // concurrency limit is full, so callers receive a gRPC
                // resource-exhausted response instead of timing out at the
                // connection's first-request guard.
                .layer(LoadShedLayer::new())
                .layer(ConcurrencyLimitLayer::new(concurrency_limit.as_usize())),
        )
        .add_routes(routes)
        .serve_with_incoming_shutdown(incoming, async move {
            let _reason = shutdown_for_server.cancelled().await;
        });

    tokio::pin!(health_sync);
    tokio::pin!(server);

    tokio::select! {
        server_result = &mut server => {
            reporter.set_service_status("", GrpcHealthServingStatus::NotServing).await;
            for service_name in service_names_for_exit {
                reporter.set_service_status(service_name, GrpcHealthServingStatus::NotServing).await;
            }
            server_result.map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))
        }
        () = &mut health_sync => {
            // The health reporter has published NOT_SERVING. Keep polling the
            // tonic server so its graceful shutdown can wait for in-flight
            // RPCs; the task supervisor owns the finite drain deadline.
            server
                .await
                .map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))
        }
    }
}

#[derive(Clone)]
pub(crate) struct GrpcServePolicy {
    pub(crate) listener_name: Arc<str>,
    pub(crate) max_concurrent_streams: Option<u32>,
    pub(crate) deadline_required: bool,
    pub(crate) max_timeout: Option<GrpcTimeout>,
    pub(crate) max_header_list_size_bytes: u32,
    pub(crate) method_policies: Vec<GrpcMethodPolicy>,
    pub(crate) rate_limit_registry: Arc<RateLimitRegistry>,
    pub(crate) trusted_proxy_headers: TrustedProxyHeaders,
    pub(crate) connection_limits: ConnectionLimitConfig,
}

/// Native gRPC method-level runtime policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcMethodPolicy {
    method: String,
    rate_limit_tier: Option<HttpRateLimitTierName>,
    max_request_message_bytes: Option<usize>,
}

impl GrpcMethodPolicy {
    /// Creates a typed method policy entry.
    pub fn new(
        method: String,
        rate_limit_tier: Option<HttpRateLimitTierName>,
        max_request_message_bytes: Option<usize>,
    ) -> Self {
        Self {
            method,
            rate_limit_tier,
            max_request_message_bytes,
        }
    }

    /// Returns the canonical gRPC method path.
    pub fn method(&self) -> &str {
        self.method.as_str()
    }

    /// Returns optional rate-limit tier hook.
    pub fn rate_limit_tier(&self) -> Option<&HttpRateLimitTierName> {
        self.rate_limit_tier.as_ref()
    }

    /// Returns optional per-method message-size cap.
    pub const fn max_request_message_bytes(&self) -> Option<usize> {
        self.max_request_message_bytes
    }
}

async fn set_registered_health_status(
    reporter: &mut crate::grpc::GrpcHealthReporter,
    readiness: &Readiness,
    service_names: &[&'static str],
) {
    let status = grpc_health_status_for_readiness(readiness);
    reporter.set_service_status("", status).await;
    for service_name in service_names {
        reporter.set_service_status(service_name, status).await;
    }
}

fn grpc_health_status_for_readiness(readiness: &Readiness) -> GrpcHealthServingStatus {
    match readiness_check(readiness).grpc_serving_status() {
        GrpcServingStatus::Serving => GrpcHealthServingStatus::Serving,
        GrpcServingStatus::NotServing => GrpcHealthServingStatus::NotServing,
    }
}

async fn sync_grpc_health_with_readiness(
    mut reporter: crate::grpc::GrpcHealthReporter,
    readiness: Readiness,
    mut watcher: ReadinessWatcher,
    service_names: Vec<&'static str>,
    mut shutdown: ShutdownToken,
) {
    loop {
        tokio::select! {
            state = watcher.changed() => {
                if state.is_err() {
                    return;
                }
                set_registered_health_status(&mut reporter, &readiness, &service_names).await;
            }
            _reason = shutdown.cancelled() => {
                reporter
                    .set_service_status("", GrpcHealthServingStatus::NotServing)
                    .await;
                for service_name in &service_names {
                    reporter
                        .set_service_status(service_name, GrpcHealthServingStatus::NotServing)
                        .await;
                }
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "grpc_tests.rs"]
mod tests;
