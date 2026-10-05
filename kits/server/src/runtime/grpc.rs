// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use futures_util::{StreamExt, stream};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::time::{Instant, sleep_until};
use tonic::service::Routes;
use tonic::transport::Server;
use tonic_health::pb::health_server::HealthServer;
use tower::ServiceBuilder;
use tower::limit::ConcurrencyLimitLayer;
use tower::load_shed::LoadShedLayer;

use super::connection_guard::{BoundedTcpListener, ForceCloseConnections};
use super::grpc_idle::{GrpcActivityLayer, GrpcConnectionActivity};
use crate::config::{
    ConnectionLimitConfig, GrpcServerConfig, RuntimeConcurrencyLimit, TrustedProxyHeaders,
};
use crate::grpc::{GrpcHealthServingStatus, health_reporter};
use crate::grpc::{GrpcTimeout, grpc_policy_layer};
use crate::health::{GrpcServingStatus, Readiness, ReadinessWatcher, readiness_check};
use crate::http::HttpRateLimitTierName;
use crate::runtime::{HttpRateLimitTierPolicy, RateLimitRegistry};
use crate::startup::TaskName;
use crate::task::{ShutdownToken, TaskExecutionError};

const DEFAULT_GRPC_HTTP2_MAX_HEADER_LIST_SIZE: u32 = 64 * 1024;
const DEFAULT_GRPC_MAX_CONCURRENT_STREAMS: u32 = 128;
#[path = "grpc/transport_timeouts.rs"]
mod transport_timeouts;
pub use transport_timeouts::{
    GrpcTransportTimeouts, GrpcTransportTimeoutsError, GrpcTransportTimeoutsErrorReason,
};

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
#[path = "grpc/serve_connections.rs"]
mod serve_connections;

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
    transport_timeouts: GrpcTransportTimeouts,
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
            transport_timeouts: GrpcTransportTimeouts::default(),
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
            transport_timeouts: GrpcTransportTimeouts::default(),
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

    /// Selects validated keepalive, age, grace, and idle transport deadlines.
    pub fn with_transport_timeouts(mut self, timeouts: GrpcTransportTimeouts) -> Self {
        self.transport_timeouts = timeouts;
        self
    }

    pub(crate) fn transport_timeouts(&self) -> GrpcTransportTimeouts {
        self.transport_timeouts
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

    let shutdown_for_server = shutdown.clone();
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
        .http2_keepalive_interval(Some(policy.transport_timeouts.keepalive_interval()))
        .http2_keepalive_timeout(Some(policy.transport_timeouts.keepalive_timeout()));
    // A per-connection shutdown signal below asks Tonic to send GOAWAY.
    // Tonic 0.14.6's age timer can otherwise force-close without GOAWAY,
    // while its no-grace age path re-polls a completed future.
    if let Some(max_timeout) = policy.max_timeout {
        builder = builder.timeout(max_timeout.as_duration());
    }
    let listener =
        BoundedTcpListener::with_force_close(listener, force_close, policy.connection_limits)
            .with_trusted_proxies(trusted_proxies)
            .with_grpc_transport_timeouts(policy.transport_timeouts);
    let service = ServiceBuilder::new()
        .layer(GrpcActivityLayer)
        .layer(grpc_policy_layer(grpc_policy))
        // The service is cloned per connection, so this semaphore remains
        // shared across the listener rather than becoming a per-peer limit.
        .layer(LoadShedLayer::new())
        .layer(ConcurrencyLimitLayer::new(concurrency_limit.as_usize()))
        .service(routes);
    let transport_timeouts = policy.transport_timeouts;
    let server = serve_connections::serve_connections(
        listener,
        move |stream, connection_shutdown| {
            let activity = stream.grpc_activity();
            // Tonic keeps its detached connection task alive after this one
            // incoming item. The pending stream lets it drain, while the
            // transport-close signal ends this future as soon as IO ends.
            let incoming = stream::once(async move { Ok::<_, std::io::Error>(stream) })
                .chain(stream::pending());
            let signal =
                grpc_connection_shutdown_signal(activity, transport_timeouts, connection_shutdown);
            Box::pin(builder.clone().serve_with_incoming_shutdown(
                service.clone(),
                incoming,
                signal,
            ))
        },
        shutdown_for_server,
    );

    tokio::pin!(health_sync);
    tokio::pin!(server);

    tokio::select! {
        server_result = &mut server => {
            reporter.set_service_status("", GrpcHealthServingStatus::NotServing).await;
            for service_name in service_names_for_exit {
                reporter.set_service_status(service_name, GrpcHealthServingStatus::NotServing).await;
            }
            server_result
        }
        () = &mut health_sync => {
            // The health reporter has published NOT_SERVING. Keep polling the
            // tonic server so its graceful shutdown can wait for in-flight
            // RPCs; the task supervisor owns the finite drain deadline.
            server.await
        }
    }
}

async fn grpc_connection_shutdown_signal(
    activity: Option<Arc<GrpcConnectionActivity>>,
    timeouts: GrpcTransportTimeouts,
    mut shutdown: ShutdownToken,
) {
    let retirement_activity = activity.clone();
    let transport_closed = async {
        match activity {
            Some(activity) => activity.wait_transport_closed().await,
            None => std::future::pending().await,
        }
    };
    tokio::select! {
        _reason = shutdown.cancelled() => {},
        () = retire_grpc_connection(retirement_activity, timeouts) => {},
        () = transport_closed => {},
    }
}

async fn retire_grpc_connection(
    activity: Option<Arc<super::grpc_idle::GrpcConnectionActivity>>,
    timeouts: GrpcTransportTimeouts,
) {
    let age_deadline = tokio::time::sleep(timeouts.max_age());
    tokio::pin!(age_deadline);
    let Some(activity) = activity else {
        age_deadline.await;
        return;
    };
    let mut idle = activity.subscribe_idle();
    loop {
        let idle_since = *idle.borrow_and_update();
        let Some(idle_since) = idle_since else {
            tokio::select! {
                _ = &mut age_deadline => return,
                changed = idle.changed() => if changed.is_err() { return },
            }
            continue;
        };
        let deadline = idle_since + timeouts.idle_timeout();
        tokio::select! {
            biased;
            changed = idle.changed() => if changed.is_err() { return },
            _ = &mut age_deadline => return,
            _ = sleep_until(deadline) => {
                if *idle.borrow() == Some(idle_since) && Instant::now() >= deadline {
                    return;
                }
            }
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
    pub(crate) transport_timeouts: GrpcTransportTimeouts,
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
