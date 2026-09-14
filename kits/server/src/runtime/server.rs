// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::sync::Arc;

use crate::config::ObservabilityConfig;
use crate::health::Readiness;
use crate::health::ReadinessState;
#[cfg(feature = "tonic-grpc")]
use crate::observability::log_grpc_listener_started;
use crate::observability::{
    RuntimeAppFailureOutcome, init_tracing, install_prometheus_recorder, log_http_listener_started,
    log_no_runtime_apps_enabled, log_observability_startup_summary, log_runtime_app_enabled,
    log_runtime_app_startup_order, log_runtime_phase_transition,
    log_runtime_startup_check_completed, log_runtime_startup_check_failed,
    log_runtime_startup_check_started, log_service_ready, log_service_starting,
    log_shutdown_completed, log_shutdown_requested, observability_startup_summary,
    record_readiness_state, record_runtime_app_startup_failure, record_runtime_phase,
    record_startup_info,
};
use crate::shutdown::{
    ShutdownError, ShutdownMode, ShutdownPolicy, ShutdownReason, shutdown_signal,
};
use crate::startup::{DeploymentRegion, ServerName, StartupBanner, TaskName, write_startup_banner};
use crate::task::{BackgroundTaskSet, ShutdownTimeout};
use crate::version::BuildInfo;

use super::app::RuntimeAppParts;
use super::background::RuntimeBackgroundTask;
use super::cleanup::run_cleanup_hooks;
use super::critical::{CriticalTaskStartError, RuntimeCriticalTask, start_critical_tasks};
use super::error::ServerRuntimeError;
#[cfg(feature = "tonic-grpc")]
use super::grpc::{GrpcServePolicy, GrpcServerSpec, serve_health_grpc};
use super::http::{HttpServerSpec, serve_http};
use super::phase::{ServerRuntimePhase, ServerRuntimePhaseReporter};
use super::rate_limit::{RateLimitRegistry, run_rate_limit_registry_sweep_task};
use super::startup_check::RuntimeStartupCheck;
use super::termination::{
    CriticalTerminationContext, RuntimeTermination, terminate_for_critical_task,
    wait_for_runtime_termination,
};

#[path = "server/composition.rs"]
mod composition;

pub use composition::ServerRuntimeBuilder;
#[cfg(feature = "tonic-grpc")]
use composition::bind_grpc_listener;
use composition::{bind_http_listener, build_runtime_http_router};

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
    critical_tasks: Vec<RuntimeCriticalTask>,
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
            critical_tasks,
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
        let critical_tasks = {
            let mut critical_tasks = critical_tasks;
            critical_tasks.extend(app_parts.critical_tasks);
            critical_tasks
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

        let mut critical_task_monitor =
            match start_critical_tasks(critical_tasks, &mut tasks, &server_name).await {
                Ok(monitor) => monitor,
                Err(CriticalTaskStartError::Registration(source)) => {
                    return Err(ServerRuntimeError::TaskRegistration { source });
                }
                Err(CriticalTaskStartError::Failure(failure)) => {
                    return Err(terminate_for_critical_task(
                        failure,
                        CriticalTerminationContext {
                            server_name: &server_name,
                            readiness: &readiness,
                            phase_reporter: &phase_reporter,
                            tasks: &mut tasks,
                            cleanup_hooks: app_parts.cleanup_hooks,
                            shutdown_timeout,
                            cleanup_timeout,
                        },
                    )
                    .await);
                }
            };

        transition_runtime_phase(&phase_reporter, &server_name, ServerRuntimePhase::Serving);
        readiness.mark_ready();
        record_readiness_state(ReadinessState::Ready);
        log_service_ready(&server_name);

        let termination =
            wait_for_runtime_termination(shutdown_signal_future, &mut critical_task_monitor)
                .await?;
        let reason = match termination {
            RuntimeTermination::Shutdown(reason) => reason,
            RuntimeTermination::CriticalTask(failure) => {
                return Err(terminate_for_critical_task(
                    failure,
                    CriticalTerminationContext {
                        server_name: &server_name,
                        readiness: &readiness,
                        phase_reporter: &phase_reporter,
                        tasks: &mut tasks,
                        cleanup_hooks: app_parts.cleanup_hooks,
                        shutdown_timeout,
                        cleanup_timeout,
                    },
                )
                .await);
            }
        };
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

#[cfg(test)]
mod tests;
