// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::service::Routes;
use tonic::transport::Server;
use tower::ServiceBuilder;
use tower::limit::ConcurrencyLimitLayer;

use crate::config::{GrpcServerConfig, RuntimeConcurrencyLimit};
use crate::grpc::{GrpcHealthServingStatus, health_reporter};
use crate::grpc::{GrpcTimeout, grpc_policy_layer};
use crate::health::{GrpcServingStatus, Readiness, readiness_check};
use crate::http::HttpRateLimitTierName;
use crate::runtime::{HttpRateLimitTierPolicy, RateLimitRegistry};
use crate::startup::TaskName;
use crate::task::{ShutdownToken, TaskExecutionError, TaskExecutionErrorKind};

const DEFAULT_GRPC_HTTP2_MAX_HEADER_LIST_SIZE: u32 = 64 * 1024;

/// gRPC server input owned by server composition and run by [`crate::runtime::ServerRuntime`].
///
/// The spec supports both health-only servers and app-provided tonic
/// routes. In both cases the runtime mounts standard gRPC health and owns the
/// listener/serve/shutdown mechanics.
///
/// Size-limit model:
/// - `max_decoding_message_bytes` is authoritative for inbound message size.
/// - Per-method `max_request_message_bytes` is an early/advisory header-based
///   reject path and is not full streaming byte accounting.
pub struct GrpcServerSpec {
    task_name: TaskName,
    config: GrpcServerConfig,
    routes: Option<Routes>,
    max_decoding_message_bytes: usize,
    max_encoding_message_bytes: usize,
    max_concurrent_streams: Option<u32>,
    deadline_required: bool,
    max_timeout: Option<GrpcTimeout>,
    max_header_list_size_bytes: u32,
    method_policies: Vec<GrpcMethodPolicy>,
    rate_limit_policies: std::sync::Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>>,
}

impl GrpcServerSpec {
    /// Creates a standard health-only gRPC server spec.
    pub fn health_only(task_name: TaskName, config: GrpcServerConfig) -> Self {
        Self {
            task_name,
            config,
            routes: None,
            max_decoding_message_bytes: crate::grpc::DEFAULT_GRPC_DECODING_MESSAGE_SIZE_BYTES,
            max_encoding_message_bytes: crate::grpc::DEFAULT_GRPC_ENCODING_MESSAGE_SIZE_BYTES,
            max_concurrent_streams: None,
            deadline_required: false,
            max_timeout: None,
            max_header_list_size_bytes: DEFAULT_GRPC_HTTP2_MAX_HEADER_LIST_SIZE,
            method_policies: Vec::new(),
            rate_limit_policies: std::sync::Arc::new(Vec::new()),
        }
    }

    /// Creates a gRPC server spec from app-owned tonic routes.
    ///
    /// The runtime will mount the standard gRPC health service alongside these
    /// routes. App crates should therefore provide product/internal RPC
    /// services only and let the runtime own health behavior.
    pub fn with_routes(task_name: TaskName, config: GrpcServerConfig, routes: Routes) -> Self {
        Self {
            task_name,
            config,
            routes: Some(routes),
            max_decoding_message_bytes: crate::grpc::DEFAULT_GRPC_DECODING_MESSAGE_SIZE_BYTES,
            max_encoding_message_bytes: crate::grpc::DEFAULT_GRPC_ENCODING_MESSAGE_SIZE_BYTES,
            max_concurrent_streams: None,
            deadline_required: false,
            max_timeout: None,
            max_header_list_size_bytes: DEFAULT_GRPC_HTTP2_MAX_HEADER_LIST_SIZE,
            method_policies: Vec::new(),
            rate_limit_policies: std::sync::Arc::new(Vec::new()),
        }
    }
    /// Attaches shared gRPC message size limits for app-owned services.
    pub fn with_message_size_limits(mut self, max_decoding: usize, max_encoding: usize) -> Self {
        self.max_decoding_message_bytes = max_decoding;
        self.max_encoding_message_bytes = max_encoding;
        self
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

    pub(crate) fn task_name(&self) -> TaskName {
        self.task_name.clone()
    }

    pub(crate) fn config(&self) -> GrpcServerConfig {
        self.config
    }

    pub(crate) fn into_routes(self) -> Option<Routes> {
        self.routes
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
    readiness: Readiness,
    policy: GrpcServePolicy,
    concurrency_limit: RuntimeConcurrencyLimit,
    shutdown: ShutdownToken,
) -> Result<(), TaskExecutionError> {
    let (mut reporter, health_service) = health_reporter();
    set_overall_health_status(&mut reporter, &readiness).await;
    let health_sync =
        sync_grpc_health_with_readiness(reporter.clone(), readiness.clone(), shutdown.clone());
    let routes = match routes {
        Some(routes) => routes.add_service(health_service),
        None => Routes::new(health_service),
    };

    let mut shutdown_for_server = shutdown.clone();
    let grpc_policy = crate::grpc::GrpcPolicy::new(
        policy.listener_name,
        policy.max_timeout,
        policy.deadline_required,
        policy.method_policies,
        policy.rate_limit_registry,
    );
    let mut builder =
        Server::builder().http2_max_header_list_size(policy.max_header_list_size_bytes);
    if let Some(max_streams) = policy.max_concurrent_streams {
        builder = builder.max_concurrent_streams(max_streams);
    }
    let server = builder
        .layer(ServiceBuilder::new().layer(grpc_policy_layer(grpc_policy)))
        .layer(ConcurrencyLimitLayer::new(concurrency_limit.as_usize()))
        .add_routes(routes)
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
            let _reason = shutdown_for_server.cancelled().await;
        });

    let mut shutdown_for_forced_stop = shutdown;

    tokio::select! {
        server_result = server => {
            health_sync.await;
            server_result.map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))
        }
        _reason = shutdown_for_forced_stop.cancelled() => {
            // Tonic/Hyper can keep an otherwise idle HTTP/2 connection alive
            // after the listener has entered graceful shutdown. Dropping the
            // server future after the runtime has requested shutdown prevents
            // an idle client channel from turning process shutdown into a
            // timeout while still letting in-flight RPC handlers observe the
            // same shutdown token through app-owned cancellation paths.
            health_sync.await;
            Ok(())
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

async fn set_overall_health_status(
    reporter: &mut crate::grpc::GrpcHealthReporter,
    readiness: &Readiness,
) {
    reporter
        .set_service_status("", grpc_health_status_for_readiness(readiness))
        .await;
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
    mut shutdown: ShutdownToken,
) {
    let mut watcher = readiness.watch();

    loop {
        tokio::select! {
            state = watcher.changed() => {
                if state.is_err() {
                    return;
                }
                set_overall_health_status(&mut reporter, &readiness).await;
            }
            _reason = shutdown.cancelled() => {
                reporter
                    .set_service_status("", GrpcHealthServingStatus::NotServing)
                    .await;
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "grpc_tests.rs"]
mod tests;
