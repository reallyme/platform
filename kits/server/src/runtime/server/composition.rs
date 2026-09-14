// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runtime builder validation and listener composition.

use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;

use crate::config::ObservabilityConfig;
use crate::health::Readiness;
use crate::http::{
    apply_standard_router_layers_with_request_logging, listener_identity_layer, operational_routes,
    route_visibility_layer_with_rate_limit_registry,
};
use crate::shutdown::ShutdownPolicy;
use crate::startup::{DeploymentRegion, ServerName, StartupBanner};
use crate::task::ShutdownTimeout;
use crate::version::BuildInfo;

use super::super::app::{RuntimeApp, collect_apps, validate_runtime_apps};
use super::super::background::RuntimeBackgroundTask;
use super::super::critical::RuntimeCriticalTask;
use super::super::error::{
    RuntimeListenerBindErrorReason, RuntimeListenerCompositionErrorReason, ServerRuntimeError,
    ServerRuntimeRequiredField, ServerRuntimeTransport,
};
#[cfg(feature = "tonic-grpc")]
use super::super::grpc::GrpcServerSpec;
use super::super::http::HttpServerSpec;
use super::super::phase::ServerRuntimePhaseReporter;
use super::super::rate_limit::RateLimitRegistry;
use super::super::startup_check::RuntimeStartupCheck;
use super::ServerRuntime;

/// Builder for [`ServerRuntime`].
#[derive(Default)]
pub struct ServerRuntimeBuilder {
    server_name: Option<ServerName>,
    deployment_region: Option<DeploymentRegion>,
    observability_config: Option<ObservabilityConfig>,
    build_info: Option<BuildInfo>,
    readiness: Option<Readiness>,
    shutdown_timeout: Option<ShutdownTimeout>,
    cleanup_timeout: Option<ShutdownTimeout>,
    fast_shutdown_timeout: Option<ShutdownTimeout>,
    shutdown_policy: Option<ShutdownPolicy>,
    startup_banner: StartupBanner,
    http_servers: Vec<HttpServerSpec>,
    #[cfg(feature = "tonic-grpc")]
    grpc_servers: Vec<GrpcServerSpec>,
    apps: Vec<RuntimeApp>,
    startup_checks: Vec<RuntimeStartupCheck>,
    background_tasks: Vec<RuntimeBackgroundTask>,
    critical_tasks: Vec<RuntimeCriticalTask>,
    phase_reporter: Option<ServerRuntimePhaseReporter>,
}

impl ServerRuntimeBuilder {
    /// Sets the validated server name.
    pub fn server_name(mut self, value: ServerName) -> Self {
        self.server_name = Some(value);
        self
    }

    /// Sets the deployment region included in safe startup summaries.
    pub fn deployment_region(mut self, value: DeploymentRegion) -> Self {
        self.deployment_region = Some(value);
        self
    }

    /// Sets observability configuration.
    pub fn observability_config(mut self, value: ObservabilityConfig) -> Self {
        self.observability_config = Some(value);
        self
    }

    /// Sets safe build/version information.
    pub fn build_info(mut self, value: BuildInfo) -> Self {
        self.build_info = Some(value);
        self
    }

    /// Sets readiness state shared by operational routes and gRPC health.
    pub fn readiness(mut self, value: Readiness) -> Self {
        self.readiness = Some(value);
        self
    }

    /// Sets the graceful shutdown drain timeout.
    pub fn shutdown_timeout(mut self, value: ShutdownTimeout) -> Self {
        self.shutdown_timeout = Some(value);
        self
    }

    /// Sets the bounded timeout for app cleanup hooks.
    pub fn cleanup_timeout(mut self, value: ShutdownTimeout) -> Self {
        self.cleanup_timeout = Some(value);
        self
    }

    /// Sets the shortened timeout used when [`ShutdownPolicy`] selects fast mode.
    pub fn fast_shutdown_timeout(mut self, value: ShutdownTimeout) -> Self {
        self.fast_shutdown_timeout = Some(value);
        self
    }

    /// Sets the mapping from shutdown reasons to graceful or fast mode.
    pub fn shutdown_policy(mut self, value: ShutdownPolicy) -> Self {
        self.shutdown_policy = Some(value);
        self
    }

    /// Sets the host-owned local-development startup banner.
    ///
    /// Omitting this value leaves the banner disabled, which is suitable for
    /// non-interactive hosts and preserves structured startup output.
    pub fn startup_banner(mut self, value: StartupBanner) -> Self {
        self.startup_banner = value;
        self
    }

    /// Adds an HTTP listener adapter.
    pub fn http_server(mut self, value: HttpServerSpec) -> Self {
        self.http_servers.push(value);
        self
    }

    /// Adds a logical runtime app hosted inside this server process.
    pub fn app(mut self, value: RuntimeApp) -> Self {
        self.apps.push(value);
        self
    }

    /// Adds a gRPC server spec.
    #[cfg(feature = "tonic-grpc")]
    pub fn grpc_server(mut self, value: GrpcServerSpec) -> Self {
        self.grpc_servers.push(value);
        self
    }

    /// Adds a app-specific startup check that must pass before readiness.
    ///
    /// Startup checks are the runtime-supported dependency/readiness gate.
    /// They run before listeners are marked serving, so failures keep the
    /// process fail-closed instead of accepting traffic with incomplete
    /// dependencies.
    pub fn startup_check(mut self, value: RuntimeStartupCheck) -> Self {
        self.startup_checks.push(value);
        self
    }

    /// Adds a app-specific background task.
    ///
    /// Background tasks are supervised and cancelled by the runtime, but they
    /// neither delay readiness nor terminate the runtime when they return. Use
    /// [`RuntimeStartupCheck`] for finite startup validation or
    /// [`RuntimeCriticalTask`] for a long-running readiness dependency.
    pub fn background_task(mut self, value: RuntimeBackgroundTask) -> Self {
        self.background_tasks.push(value);
        self
    }

    /// Adds a critical task that gates readiness and must remain active.
    pub fn critical_task(mut self, value: RuntimeCriticalTask) -> Self {
        self.critical_tasks.push(value);
        self
    }

    /// Sets the lifecycle phase reporter used by the runtime.
    pub fn phase_reporter(mut self, value: ServerRuntimePhaseReporter) -> Self {
        self.phase_reporter = Some(value);
        self
    }

    /// Builds the runtime after validating required fields.
    pub fn build(self) -> Result<ServerRuntime, ServerRuntimeError> {
        let server_name = required(self.server_name, ServerRuntimeRequiredField::ServerName)?;
        let observability_config = required(
            self.observability_config,
            ServerRuntimeRequiredField::ObservabilityConfig,
        )?;
        let build_info = required(self.build_info, ServerRuntimeRequiredField::BuildInfo)?;
        let readiness = required(self.readiness, ServerRuntimeRequiredField::Readiness)?;
        let shutdown_timeout = required(
            self.shutdown_timeout,
            ServerRuntimeRequiredField::ShutdownTimeout,
        )?;
        validate_runtime_apps(self.apps.as_slice())?;
        let cleanup_timeout = self.cleanup_timeout.unwrap_or(shutdown_timeout);
        let fast_shutdown_timeout = self.fast_shutdown_timeout.unwrap_or(shutdown_timeout);
        validate_listener_ports(
            self.http_servers.as_slice(),
            #[cfg(feature = "tonic-grpc")]
            self.grpc_servers.as_slice(),
        )?;
        let app_parts = collect_apps(self.apps)?;

        Ok(ServerRuntime {
            server_name,
            deployment_region: self.deployment_region.unwrap_or_default(),
            observability_config,
            build_info,
            readiness,
            shutdown_timeout,
            cleanup_timeout,
            fast_shutdown_timeout,
            shutdown_policy: self.shutdown_policy.unwrap_or_default(),
            startup_banner: self.startup_banner,
            http_servers: self.http_servers,
            #[cfg(feature = "tonic-grpc")]
            grpc_servers: self.grpc_servers,
            app_parts,
            startup_checks: self.startup_checks,
            background_tasks: self.background_tasks,
            critical_tasks: self.critical_tasks,
            phase_reporter: self.phase_reporter.unwrap_or_default(),
        })
    }

    /// Builds and runs the runtime.
    pub async fn run(self) -> Result<(), ServerRuntimeError> {
        self.build()?.run().await
    }
}

fn required<T>(
    value: Option<T>,
    field: ServerRuntimeRequiredField,
) -> Result<T, ServerRuntimeError> {
    value.ok_or(ServerRuntimeError::MissingRequiredField { field })
}

fn validate_listener_ports(
    http_servers: &[HttpServerSpec],
    #[cfg(feature = "tonic-grpc")] grpc_servers: &[GrpcServerSpec],
) -> Result<(), ServerRuntimeError> {
    let listener_addresses_overlap = |left: &HttpServerSpec, right: &HttpServerSpec| {
        let left_bind_address = left.config().bind_address().as_socket_addr();
        let right_bind_address = right.config().bind_address().as_socket_addr();

        if left_bind_address.port() == 0 || right_bind_address.port() == 0 {
            return false;
        }

        left_bind_address == right_bind_address
    };

    for (left_index, left) in http_servers.iter().enumerate() {
        for right in http_servers.iter().skip(left_index + 1) {
            if left.name() == right.name() {
                return Err(ServerRuntimeError::ListenerComposition {
                    reason: RuntimeListenerCompositionErrorReason::DuplicateHttpListenerName,
                });
            }

            if listener_addresses_overlap(left, right) {
                return Err(ServerRuntimeError::ListenerComposition {
                    reason: RuntimeListenerCompositionErrorReason::DuplicateListenerPort,
                });
            }
        }

        #[cfg(feature = "tonic-grpc")]
        {
            for grpc_server in grpc_servers {
                let grpc_bind_address = grpc_server.config().bind_address().as_socket_addr();
                let http_bind_address = left.config().bind_address().as_socket_addr();

                if grpc_bind_address.port() == 0 || http_bind_address.port() == 0 {
                    continue;
                }

                if grpc_bind_address == http_bind_address {
                    return Err(ServerRuntimeError::ListenerComposition {
                        reason: RuntimeListenerCompositionErrorReason::DuplicateListenerPort,
                    });
                }
            }
        }
    }

    #[cfg(feature = "tonic-grpc")]
    for (left_index, left) in grpc_servers.iter().enumerate() {
        for right in grpc_servers.iter().skip(left_index + 1) {
            let left_bind_address = left.config().bind_address().as_socket_addr();
            let right_bind_address = right.config().bind_address().as_socket_addr();

            if left_bind_address.port() == 0 || right_bind_address.port() == 0 {
                continue;
            }

            if right_bind_address == left_bind_address {
                return Err(ServerRuntimeError::ListenerComposition {
                    reason: RuntimeListenerCompositionErrorReason::DuplicateListenerPort,
                });
            }
        }
    }

    Ok(())
}

pub(super) async fn bind_http_listener(
    http_server: &HttpServerSpec,
) -> Result<TcpListener, ServerRuntimeError> {
    let bind_address = http_server.config().bind_address().as_socket_addr();
    TcpListener::bind(bind_address)
        .await
        .map_err(|error| ServerRuntimeError::ListenerBind {
            transport: ServerRuntimeTransport::Http,
            bind_address,
            reason: RuntimeListenerBindErrorReason::from_io_error_kind(error.kind()),
        })
}

#[cfg(feature = "tonic-grpc")]
pub(super) async fn bind_grpc_listener(
    grpc_server: &GrpcServerSpec,
) -> Result<TcpListener, ServerRuntimeError> {
    let bind_address = grpc_server.config().bind_address().as_socket_addr();
    TcpListener::bind(bind_address)
        .await
        .map_err(|error| ServerRuntimeError::ListenerBind {
            transport: ServerRuntimeTransport::Grpc,
            bind_address,
            reason: RuntimeListenerBindErrorReason::from_io_error_kind(error.kind()),
        })
}

pub(super) fn build_runtime_http_router(
    http_server: HttpServerSpec,
    request_logging: crate::config::HttpRequestLoggingConfig,
    readiness: Readiness,
    build_info: BuildInfo,
    metrics: crate::observability::MetricsExporter,
    rate_limit_registry: Arc<RateLimitRegistry>,
    app_router: Router,
) -> Router {
    let config = http_server.config().clone();
    let local_socket_addr = config.bind_address().as_socket_addr();
    let listener_name = http_server.name().clone();
    let visibility = http_server.visibility();
    let route_visibility_policy = http_server.route_visibility_policy().clone();
    let listener_rate_limit_tier = http_server.rate_limit_tier().cloned();
    let router = operational_routes(readiness, build_info, metrics)
        .merge(app_router)
        .merge(http_server.into_router());

    apply_standard_router_layers_with_request_logging(
        router.layer(route_visibility_layer_with_rate_limit_registry(
            listener_name.clone(),
            visibility.clone(),
            listener_rate_limit_tier,
            rate_limit_registry,
            &route_visibility_policy,
        )),
        &config,
        request_logging,
    )
    .layer(listener_identity_layer(
        listener_name,
        visibility,
        local_socket_addr,
    ))
}
