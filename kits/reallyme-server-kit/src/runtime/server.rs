// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

use axum::Router;
use tokio::net::TcpListener;

use crate::config::ObservabilityConfig;
use crate::health::Readiness;
use crate::health::ReadinessState;
use crate::http::{
    apply_standard_router_layers_with_request_logging, listener_identity_layer, operational_routes,
    route_visibility_layer_with_rate_limit_registry,
};
#[cfg(feature = "tonic-grpc")]
use crate::observability::log_grpc_listener_started;
use crate::observability::{
    RuntimeAppFailureOutcome, init_tracing, install_prometheus_recorder, log_http_listener_started,
    log_no_runtime_apps_enabled, log_observability_startup_summary,
    log_runtime_app_cleanup_completed, log_runtime_app_cleanup_failed,
    log_runtime_app_cleanup_started, log_runtime_app_enabled, log_runtime_app_startup_order,
    log_runtime_phase_transition, log_runtime_startup_check_completed,
    log_runtime_startup_check_failed, log_runtime_startup_check_started, log_service_ready,
    log_service_starting, log_shutdown_completed, log_shutdown_requested,
    observability_startup_summary, record_readiness_state, record_runtime_app_cleanup_failure,
    record_runtime_app_startup_failure, record_runtime_phase, record_startup_info,
};
use crate::shutdown::{
    ShutdownError, ShutdownMode, ShutdownPolicy, ShutdownReason, shutdown_signal,
};
use crate::startup::{DeploymentRegion, ServerName, StartupBanner, TaskName, write_startup_banner};
use crate::task::{BackgroundTaskSet, ShutdownTimeout};
use crate::version::BuildInfo;

use super::app::{RuntimeApp, RuntimeAppParts, collect_apps, validate_runtime_apps};
use super::background::RuntimeBackgroundTask;
use super::cleanup::RuntimeCleanupResult;
use super::error::{
    RuntimeAppCleanupErrorReason, RuntimeListenerBindErrorReason,
    RuntimeListenerCompositionErrorReason, ServerRuntimeError, ServerRuntimeRequiredField,
    ServerRuntimeTransport,
};
#[cfg(feature = "tonic-grpc")]
use super::grpc::{GrpcServePolicy, GrpcServerSpec, serve_health_grpc};
use super::http::{HttpServerSpec, serve_http};
use super::phase::{ServerRuntimePhase, ServerRuntimePhaseReporter};
use super::rate_limit::{RateLimitRegistry, run_rate_limit_registry_sweep_task};
use super::startup_check::RuntimeStartupCheck;

/// Reusable runtime for a ReallyMe server process.
pub struct ServerRuntime {
    server_name: ServerName,
    deployment_region: DeploymentRegion,
    observability_config: ObservabilityConfig,
    build_info: BuildInfo,
    readiness: Readiness,
    shutdown_timeout: ShutdownTimeout,
    cleanup_timeout: ShutdownTimeout,
    fast_shutdown_timeout: ShutdownTimeout,
    shutdown_policy: ShutdownPolicy,
    startup_banner: StartupBanner,
    http_servers: Vec<HttpServerSpec>,
    #[cfg(feature = "tonic-grpc")]
    grpc_servers: Vec<GrpcServerSpec>,
    app_parts: RuntimeAppParts,
    startup_checks: Vec<RuntimeStartupCheck>,
    background_tasks: Vec<RuntimeBackgroundTask>,
    phase_reporter: ServerRuntimePhaseReporter,
}

impl ServerRuntime {
    /// Creates a new server runtime builder.
    pub fn builder() -> ServerRuntimeBuilder {
        ServerRuntimeBuilder::default()
    }

    /// Runs the server process until an operating-system shutdown signal is received.
    pub async fn run(self) -> Result<(), ServerRuntimeError> {
        self.run_until_shutdown(shutdown_signal()).await
    }

    async fn run_until_shutdown<F>(
        self,
        shutdown_signal_future: F,
    ) -> Result<(), ServerRuntimeError>
    where
        F: Future<Output = Result<ShutdownReason, ShutdownError>>,
    {
        let server_name = self.server_name.clone();
        let phase_reporter = self.phase_reporter.clone();
        let result = self.run_inner(shutdown_signal_future).await;

        if result.is_err() {
            transition_runtime_phase(&phase_reporter, &server_name, ServerRuntimePhase::Failed);
        }

        result
    }

    async fn run_inner<F>(self, shutdown_signal_future: F) -> Result<(), ServerRuntimeError>
    where
        F: Future<Output = Result<ShutdownReason, ShutdownError>>,
    {
        let Self {
            server_name,
            deployment_region,
            observability_config,
            build_info,
            readiness,
            shutdown_timeout,
            cleanup_timeout,
            fast_shutdown_timeout,
            shutdown_policy,
            startup_banner,
            http_servers,
            #[cfg(feature = "tonic-grpc")]
            grpc_servers,
            app_parts,
            startup_checks,
            background_tasks,
            phase_reporter,
        } = self;

        {
            // Keep the stdout lock scoped to the banner write only. Holding it
            // for the full runtime lifetime would deadlock structured tracing
            // writes on request completion and during shutdown.
            let mut stdout = std::io::stdout().lock();
            write_startup_banner(
                &mut stdout,
                startup_banner,
                observability_config.service_environment(),
                observability_config.log_format(),
            )
            .map_err(|_| ServerRuntimeError::StartupBannerWriteFailed)?;
        }

        init_tracing(server_name.clone(), &observability_config)?;
        log_service_starting(&server_name, &build_info);
        log_observability_startup_summary(&observability_startup_summary(
            &server_name,
            deployment_region.clone(),
            &observability_config,
        ));
        if app_parts.ordered_app_names.is_empty() {
            log_no_runtime_apps_enabled(&server_name);
        } else {
            for app_name in &app_parts.ordered_app_names {
                log_runtime_app_enabled(&server_name, app_name);
            }
        }

        for (startup_order, app_name) in app_parts.ordered_app_names.iter().enumerate() {
            log_runtime_app_startup_order(&server_name, app_name, startup_order);
        }
        let startup_checks = {
            let mut startup_checks = startup_checks;
            startup_checks.extend(app_parts.startup_checks);
            startup_checks
        };
        let background_tasks = {
            let mut background_tasks = background_tasks;
            background_tasks.extend(app_parts.background_tasks);
            background_tasks
        };

        let metrics = install_prometheus_recorder(&observability_config)?;
        record_startup_info(&server_name, &build_info);
        publish_runtime_phase(&server_name, ServerRuntimePhase::Initializing);

        let mut tasks = BackgroundTaskSet::new();
        let process_shutdown_token = tasks.shutdown_token();
        for websocket_shutdown_consumer in app_parts.websocket_shutdown_consumers {
            websocket_shutdown_consumer(process_shutdown_token.clone());
        }

        for startup_check in startup_checks {
            let check_name = startup_check.name();
            log_runtime_startup_check_started(&server_name, &check_name);
            match startup_check.run().await {
                Ok(()) => log_runtime_startup_check_completed(&server_name, &check_name),
                Err(kind) => {
                    record_runtime_app_startup_failure(RuntimeAppFailureOutcome::Failed);
                    log_runtime_startup_check_failed(&server_name, &check_name, kind);
                    return Err(ServerRuntimeError::StartupCheck { check_name, kind });
                }
            }
        }

        transition_runtime_phase(
            &phase_reporter,
            &server_name,
            ServerRuntimePhase::BindingListeners,
        );

        for http_server in http_servers {
            let listener = bind_http_listener(&http_server).await?;
            log_http_listener_started(&server_name, http_server.config().bind_address());
            let task_name = TaskName::new(format!("http-{}", http_server.name().as_str()))?;
            let listener_name = http_server.name().clone();
            let rate_limit_policies = http_server.rate_limit_policies();
            let rate_limit_registry =
                Arc::new(RateLimitRegistry::new(Arc::clone(&rate_limit_policies)));
            let listener_name_for_sweep = listener_name.clone();
            let router = build_runtime_http_router(
                http_server,
                observability_config.request_logging(),
                readiness.clone(),
                build_info.clone(),
                metrics.clone(),
                rate_limit_registry.clone(),
                app_parts.http_router.clone(),
            );

            tasks
                .spawn_fallible(task_name, move |shutdown| {
                    serve_http(listener, router, listener_name, shutdown)
                })
                .map_err(|source| ServerRuntimeError::TaskRegistration { source })?;

            let sweep_task_name = TaskName::new(format!(
                "http-rate-limit-sweep-{}",
                listener_name_for_sweep.as_str()
            ))?;
            let sweep_listener_name = listener_name_for_sweep.clone();
            tasks
                .spawn_fallible(sweep_task_name, move |shutdown| {
                    run_rate_limit_registry_sweep_task(
                        rate_limit_registry,
                        sweep_listener_name.clone_shared(),
                        shutdown,
                    )
                })
                .map_err(|source| ServerRuntimeError::TaskRegistration { source })?;
        }

        #[cfg(feature = "tonic-grpc")]
        {
            for grpc_server in grpc_servers {
                let listener = bind_grpc_listener(&grpc_server).await?;
                let task_name = grpc_server.task_name();
                log_grpc_listener_started(
                    &server_name,
                    &task_name,
                    grpc_server.config().bind_address(),
                );
                let concurrency_limit = grpc_server.config().concurrency_limit();
                let max_concurrent_streams = grpc_server.max_concurrent_streams();
                let deadline_required = grpc_server.deadline_required();
                let max_timeout = grpc_server.max_timeout();
                let max_header_list_size_bytes = grpc_server.max_header_list_size_bytes();
                let method_policies = grpc_server.method_policies();
                let rate_limit_policies = grpc_server.rate_limit_policies();
                let rate_limit_registry =
                    Arc::new(RateLimitRegistry::new(rate_limit_policies.clone()));
                let rate_limit_registry_for_policy = Arc::clone(&rate_limit_registry);
                let routes = grpc_server.into_routes();
                let readiness = readiness.clone();
                let listener_name = std::sync::Arc::<str>::from(task_name.as_str());
                let sweep_listener_name = Arc::clone(&listener_name);

                tasks
                    .spawn_fallible(task_name, move |shutdown| {
                        serve_health_grpc(
                            listener,
                            routes,
                            readiness,
                            GrpcServePolicy {
                                listener_name,
                                max_concurrent_streams,
                                deadline_required,
                                max_timeout,
                                max_header_list_size_bytes,
                                method_policies,
                                rate_limit_registry: rate_limit_registry_for_policy,
                            },
                            concurrency_limit,
                            shutdown,
                        )
                    })
                    .map_err(|source| ServerRuntimeError::TaskRegistration { source })?;

                let sweep_task_name =
                    TaskName::new(format!("grpc-rate-limit-sweep-{}", sweep_listener_name))?;
                tasks
                    .spawn_fallible(sweep_task_name, move |shutdown| {
                        run_rate_limit_registry_sweep_task(
                            rate_limit_registry,
                            sweep_listener_name,
                            shutdown,
                        )
                    })
                    .map_err(|source| ServerRuntimeError::TaskRegistration { source })?;
            }
        }

        transition_runtime_phase(
            &phase_reporter,
            &server_name,
            ServerRuntimePhase::StartingBackgroundTasks,
        );

        for background_task in background_tasks {
            let task_name = background_task.task_name();
            let task = background_task.into_task();

            tasks
                .spawn_fallible(task_name, task)
                .map_err(|source| ServerRuntimeError::TaskRegistration { source })?;
        }

        transition_runtime_phase(&phase_reporter, &server_name, ServerRuntimePhase::Serving);
        readiness.mark_ready();
        record_readiness_state(ReadinessState::Ready);
        log_service_ready(&server_name);

        let reason = shutdown_signal_future
            .await
            .map_err(|source| ServerRuntimeError::Shutdown { source })?;
        let shutdown_mode = shutdown_policy.mode_for(reason);
        let active_shutdown_timeout = match shutdown_mode {
            ShutdownMode::Graceful => shutdown_timeout,
            ShutdownMode::Fast => fast_shutdown_timeout,
        };
        readiness.mark_not_ready();
        record_readiness_state(ReadinessState::NotReady);
        log_shutdown_requested(&server_name, reason);
        transition_runtime_phase(&phase_reporter, &server_name, ServerRuntimePhase::Draining);

        tasks
            .shutdown(reason, active_shutdown_timeout)
            .await
            .map_err(|source| ServerRuntimeError::Shutdown { source })?;

        run_cleanup_hooks(
            app_parts.cleanup_hooks,
            match shutdown_mode {
                ShutdownMode::Graceful => cleanup_timeout,
                ShutdownMode::Fast => active_shutdown_timeout,
            },
            &server_name,
        )
        .await?;
        transition_runtime_phase(
            &phase_reporter,
            &server_name,
            ServerRuntimePhase::ShuttingDown,
        );
        log_shutdown_completed(&server_name, reason);
        transition_runtime_phase(&phase_reporter, &server_name, ServerRuntimePhase::Stopped);

        Ok(())
    }
}

fn transition_runtime_phase(
    phase_reporter: &ServerRuntimePhaseReporter,
    server_name: &ServerName,
    phase: ServerRuntimePhase,
) {
    phase_reporter.transition(phase);
    publish_runtime_phase(server_name, phase);
}

fn publish_runtime_phase(server_name: &ServerName, phase: ServerRuntimePhase) {
    log_runtime_phase_transition(server_name, phase);
    record_runtime_phase(phase);
}

async fn run_cleanup_hooks(
    cleanup_hooks: Vec<super::app::RuntimeAppCleanup>,
    cleanup_timeout: ShutdownTimeout,
    server_name: &ServerName,
) -> Result<(), ServerRuntimeError> {
    let mut first_error = None;
    let cleanup_deadline = Instant::now() + cleanup_timeout.as_duration();

    for cleanup in cleanup_hooks.into_iter().rev() {
        let hook_name = cleanup.hook.name();

        let now = Instant::now();
        if now >= cleanup_deadline {
            first_error.get_or_insert(ServerRuntimeError::AppCleanup {
                hook_name: hook_name.clone(),
                reason: RuntimeAppCleanupErrorReason::TimedOut,
            });

            record_runtime_app_cleanup_failure(RuntimeAppFailureOutcome::TimedOut);
            log_runtime_app_cleanup_failed(
                server_name,
                &cleanup.app_name,
                &hook_name,
                crate::task::TaskExecutionErrorKind::Internal,
            );

            break;
        }

        let mut remaining_timeout = cleanup_deadline.duration_since(now);
        if remaining_timeout.is_zero() {
            remaining_timeout = std::time::Duration::from_nanos(1);
        }
        let remaining_timeout = match ShutdownTimeout::new(remaining_timeout) {
            Ok(timeout) => timeout,
            Err(_) => {
                first_error.get_or_insert(ServerRuntimeError::AppCleanup {
                    hook_name: hook_name.clone(),
                    reason: RuntimeAppCleanupErrorReason::TimedOut,
                });

                record_runtime_app_cleanup_failure(RuntimeAppFailureOutcome::TimedOut);
                log_runtime_app_cleanup_failed(
                    server_name,
                    &cleanup.app_name,
                    &hook_name,
                    crate::task::TaskExecutionErrorKind::Internal,
                );

                break;
            }
        };

        log_runtime_app_cleanup_started(server_name, &cleanup.app_name, &hook_name);

        match cleanup.hook.run(remaining_timeout).await {
            RuntimeCleanupResult::Completed => {
                log_runtime_app_cleanup_completed(server_name, &cleanup.app_name, &hook_name);
            }
            RuntimeCleanupResult::Failed { kind } => {
                record_runtime_app_cleanup_failure(RuntimeAppFailureOutcome::Failed);
                log_runtime_app_cleanup_failed(server_name, &cleanup.app_name, &hook_name, kind);
                first_error.get_or_insert(ServerRuntimeError::AppCleanup {
                    hook_name,
                    reason: RuntimeAppCleanupErrorReason::Failed { kind },
                });
            }
            RuntimeCleanupResult::TimedOut { timeout: _ } => {
                record_runtime_app_cleanup_failure(RuntimeAppFailureOutcome::TimedOut);
                log_runtime_app_cleanup_failed(
                    server_name,
                    &cleanup.app_name,
                    &hook_name,
                    crate::task::TaskExecutionErrorKind::Internal,
                );
                first_error.get_or_insert(ServerRuntimeError::AppCleanup {
                    hook_name,
                    reason: RuntimeAppCleanupErrorReason::TimedOut,
                });
            }
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

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
    /// do not delay readiness. Use [`RuntimeStartupCheck`] for initialization
    /// that must complete before traffic is accepted.
    pub fn background_task(mut self, value: RuntimeBackgroundTask) -> Self {
        self.background_tasks.push(value);
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

async fn bind_http_listener(
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
async fn bind_grpc_listener(
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

fn build_runtime_http_router(
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

#[cfg(test)]
mod tests;
