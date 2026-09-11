// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Minimal server skeleton showing how a ReallyMe service wires server-kit.
//!
//! This example intentionally stops before binding a socket. Deployable
//! services should own their environment variable names and map them into the
//! validated server-kit runtime config types shown here.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use axum::Router;
use reallyme_server_kit::config::{
    BindAddress, BodyLimitConfig, ConfigError, CorsConfig, HttpServerConfig, LogFormat,
    MetricsIdleTimeout, ObservabilityConfig, RequestBodyLimitBytes, RequestTimeout,
    ServiceEnvironment, TimeoutConfig,
};
use reallyme_server_kit::health::Readiness;
use reallyme_server_kit::http::apply_standard_router_layers;
use reallyme_server_kit::http::operational_routes;
use reallyme_server_kit::observability::{
    ObservabilityError, init_tracing, install_prometheus_recorder,
    log_observability_startup_summary, log_service_starting, observability_startup_summary,
};
use reallyme_server_kit::shutdown::ShutdownError;
use reallyme_server_kit::startup::{DeploymentRegion, ServerName, StartupError, TaskName};
use reallyme_server_kit::task::{BackgroundTaskSet, ShutdownTimeout};
use reallyme_server_kit::version::BuildInfo;
use thiserror::Error;

fn main() -> Result<(), ExampleServiceError> {
    let runtime = tokio::runtime::Runtime::new().map_err(|_| ExampleServiceError::Runtime)?;

    runtime.block_on(run())
}

async fn run() -> Result<(), ExampleServiceError> {
    let server_name = ServerName::new("reallyme-example-service")?;
    let build_info = BuildInfo::new(server_name.clone());
    let observability = observability_config()?;

    init_tracing(server_name.clone(), &observability)?;
    log_service_starting(&server_name, &build_info);
    log_observability_startup_summary(&observability_startup_summary(
        &server_name,
        DeploymentRegion::default(),
        &observability,
    ));

    let metrics = install_prometheus_recorder(&observability)?;
    let readiness = Readiness::new();
    let http = http_config()?;
    let router: Router = operational_routes(readiness.clone(), build_info, metrics);
    let _router = apply_standard_router_layers(router, &http);

    let mut tasks = BackgroundTaskSet::new();
    tasks.spawn(
        TaskName::new("example-background-worker")?,
        |mut shutdown| async move {
            let _reason = shutdown.cancelled().await;
        },
    )?;

    readiness.mark_ready();

    readiness.mark_not_ready();
    let shutdown_timeout = ShutdownTimeout::new(Duration::from_secs(5))?;
    tasks
        .shutdown(
            reallyme_server_kit::shutdown::ShutdownReason::Unknown,
            shutdown_timeout,
        )
        .await?;

    Ok(())
}

fn http_config() -> Result<HttpServerConfig, ConfigError> {
    let bind_address =
        BindAddress::new(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080)))?;
    let timeout = TimeoutConfig::new(RequestTimeout::new(Duration::from_secs(5))?);
    let body_limit = BodyLimitConfig::new(RequestBodyLimitBytes::new(1_048_576)?);

    Ok(HttpServerConfig::new(
        bind_address,
        CorsConfig::no_cors(),
        timeout,
        body_limit,
    ))
}

fn observability_config() -> Result<ObservabilityConfig, ConfigError> {
    ObservabilityConfig::new(
        ServiceEnvironment::Local,
        LogFormat::PlainText,
        false,
        "reallyme_server_kit=info".to_owned(),
        MetricsIdleTimeout::new(Duration::from_secs(30))?,
    )
}

#[derive(Debug, Error)]
enum ExampleServiceError {
    #[error("tokio runtime initialization failed")]
    Runtime,
    #[error(transparent)]
    Startup(#[from] StartupError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Observability(#[from] ObservabilityError),
    #[error(transparent)]
    Shutdown(#[from] ShutdownError),
}
