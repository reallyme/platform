// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use axum::Router;
#[cfg(feature = "tonic-grpc")]
use axum::http::StatusCode;
#[cfg(feature = "tonic-grpc")]
use axum::routing::get;

#[cfg(feature = "tonic-grpc")]
use super::fixtures::{grpc_server_config, http_server_config};
use super::fixtures::{http_server_config_for_port, observability_config};
use crate::config::{
    BindAddress, BodyLimitConfig, CorsConfig, HttpServerConfig, RequestBodyLimitBytes,
    RequestTimeout, TimeoutConfig,
};
use crate::health::Readiness;
use crate::http::{HttpListenerName, HttpListenerVisibility};
use crate::runtime::{
    AppHttpMountPath, AppName, HttpServerSpec, RuntimeApp, RuntimeAppCompositionErrorReason,
    RuntimeListenerCompositionErrorReason, ServerRuntime, ServerRuntimeError,
    ServerRuntimeRequiredField, ServerRuntimeTransport,
};
#[cfg(feature = "tonic-grpc")]
use crate::runtime::{
    GrpcServerSpec, RuntimeBackgroundTask, RuntimeStartupCheck, ServerRuntimePhase,
    ServerRuntimePhaseReporter,
};
use crate::startup::ServerName;
#[cfg(feature = "tonic-grpc")]
use crate::startup::TaskName;
use crate::task::ShutdownTimeout;
#[cfg(feature = "tonic-grpc")]
use crate::task::TaskExecutionError;
use crate::version::BuildInfo;

#[test]
fn builder_rejects_missing_required_fields() {
    let result = ServerRuntime::builder().build();

    assert!(matches!(
        result,
        Err(ServerRuntimeError::MissingRequiredField {
            field: ServerRuntimeRequiredField::ServerName,
        })
    ));
}

#[test]
fn runtime_transport_error_kinds_are_stable() {
    assert_eq!(ServerRuntimeTransport::Http, ServerRuntimeTransport::Http);
    assert_eq!(ServerRuntimeTransport::Grpc, ServerRuntimeTransport::Grpc);
}

#[test]
fn builder_rejects_duplicate_runtime_app_names() {
    let server_name = ServerName::new("duplicate-app-fixture").expect("valid fixture server name");
    let first_app = RuntimeApp::new(
        AppName::new("api").expect("valid fixture app name"),
        Router::new(),
    );
    let second_app = RuntimeApp::new(
        AppName::new("api").expect("valid fixture app name"),
        Router::new(),
    )
    .with_http_mount(AppHttpMountPath::new("/second").expect("valid fixture HTTP mount path"));

    let result = ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(observability_config())
        .build_info(BuildInfo::new(server_name))
        .readiness(Readiness::new())
        .shutdown_timeout(
            ShutdownTimeout::new(Duration::from_secs(5)).expect("valid fixture shutdown timeout"),
        )
        .app(first_app)
        .app(second_app)
        .build();

    assert!(matches!(
        result,
        Err(ServerRuntimeError::AppComposition {
            reason: RuntimeAppCompositionErrorReason::DuplicateAppName,
        })
    ));
}

#[test]
#[cfg(feature = "tonic-grpc")]
fn builder_rejects_duplicate_listener_ports_between_http_and_grpc() {
    let server_name = ServerName::new("duplicate-port-fixture").expect("valid fixture server name");

    let result = ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(observability_config())
        .build_info(BuildInfo::new(server_name))
        .readiness(Readiness::new())
        .shutdown_timeout(
            ShutdownTimeout::new(Duration::from_secs(5)).expect("valid fixture shutdown timeout"),
        )
        .http_server(HttpServerSpec::new(
            http_server_config_for_port(18_080),
            Router::new(),
        ))
        .grpc_server(GrpcServerSpec::health_only(
            TaskName::new("duplicate-port-grpc").expect("valid fixture task name"),
            crate::config::GrpcServerConfig::new(super::fixtures::bind_address(18_080)),
        ))
        .build();

    assert!(matches!(
        result,
        Err(ServerRuntimeError::ListenerComposition {
            reason: RuntimeListenerCompositionErrorReason::DuplicateListenerPort,
        })
    ));
}

#[test]
#[cfg(feature = "tonic-grpc")]
fn builder_rejects_duplicate_listener_ports_between_grpc_specs() {
    let server_name =
        ServerName::new("duplicate-grpc-port-fixture").expect("valid fixture server name");

    let result = ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(observability_config())
        .build_info(BuildInfo::new(server_name))
        .readiness(Readiness::new())
        .shutdown_timeout(
            ShutdownTimeout::new(Duration::from_secs(5)).expect("valid fixture shutdown timeout"),
        )
        .grpc_server(GrpcServerSpec::health_only(
            TaskName::new("first-grpc").expect("valid fixture task name"),
            crate::config::GrpcServerConfig::new(super::fixtures::bind_address(15_051)),
        ))
        .grpc_server(GrpcServerSpec::health_only(
            TaskName::new("second-grpc").expect("valid fixture task name"),
            crate::config::GrpcServerConfig::new(super::fixtures::bind_address(15_051)),
        ))
        .build();

    assert!(matches!(
        result,
        Err(ServerRuntimeError::ListenerComposition {
            reason: RuntimeListenerCompositionErrorReason::DuplicateListenerPort,
        })
    ));
}

#[test]
fn builder_rejects_duplicate_listener_ports_for_exact_socket_match_only() {
    let server_name =
        ServerName::new("duplicate-socket-fixture").expect("valid fixture server name");

    let result = ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(observability_config())
        .build_info(BuildInfo::new(server_name))
        .readiness(Readiness::new())
        .shutdown_timeout(
            ShutdownTimeout::new(Duration::from_secs(5)).expect("valid fixture shutdown timeout"),
        )
        .http_server(HttpServerSpec::with_listener(
            HttpListenerName::new("socket-a").expect("valid fixture listener name"),
            HttpListenerVisibility::Public,
            http_server_config_with_socket(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::LOCALHOST,
                18_080,
            ))),
            Router::new(),
        ))
        .http_server(HttpServerSpec::with_listener(
            HttpListenerName::new("socket-b").expect("valid fixture listener name"),
            HttpListenerVisibility::Public,
            http_server_config_with_socket(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(192, 168, 1, 10),
                18_080,
            ))),
            Router::new(),
        ))
        .build();

    assert!(result.is_ok());
}

#[test]
fn builder_rejects_exact_duplicate_listener_addresses() {
    let server_name =
        ServerName::new("duplicate-address-fixture").expect("valid fixture server name");

    let result = ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(observability_config())
        .build_info(BuildInfo::new(server_name))
        .readiness(Readiness::new())
        .shutdown_timeout(
            ShutdownTimeout::new(Duration::from_secs(5)).expect("valid fixture shutdown timeout"),
        )
        .http_server(HttpServerSpec::with_listener(
            HttpListenerName::new("socket-dupe-a").expect("valid fixture listener name"),
            HttpListenerVisibility::Public,
            http_server_config_for_port(18_080),
            Router::new(),
        ))
        .http_server(HttpServerSpec::with_listener(
            HttpListenerName::new("socket-dupe-b").expect("valid fixture listener name"),
            HttpListenerVisibility::Public,
            http_server_config_with_socket(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::LOCALHOST,
                18_080,
            ))),
            Router::new(),
        ))
        .build();

    assert!(matches!(
        result,
        Err(ServerRuntimeError::ListenerComposition {
            reason: RuntimeListenerCompositionErrorReason::DuplicateListenerPort,
        })
    ));
}

#[test]
#[cfg(feature = "tonic-grpc")]
fn hypothetical_second_app_can_compose_runtime_without_reimplementing_lifecycle() {
    let server_name =
        ServerName::new("reallyme-second-fixture").expect("valid fixture server name");
    let http_router = Router::new().route(
        "/second-fixture/ping",
        get(|| async { StatusCode::NO_CONTENT }),
    );
    let grpc_routes = tonic::service::Routes::from(Router::new());
    let (phase_reporter, phase_watch) = ServerRuntimePhaseReporter::new();
    let startup_check = RuntimeStartupCheck::new(
        TaskName::new("second-fixture-startup-check").expect("valid fixture task name"),
        || async { Ok::<(), TaskExecutionError>(()) },
    );
    let noop_background_task = RuntimeBackgroundTask::new(
        TaskName::new("second-fixture-noop").expect("valid fixture task name"),
        |_shutdown| async { Ok::<(), TaskExecutionError>(()) },
    );

    // This fixture intentionally does not call `run()`: the production
    // runtime waits for an OS shutdown signal by design. Building the spec
    // is enough to prove another app can provide only its identity, own HTTP
    // routes, optional gRPC routes, and managed tasks.
    let result = ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(observability_config())
        .build_info(BuildInfo::new(server_name))
        .readiness(Readiness::new())
        .shutdown_timeout(
            ShutdownTimeout::new(Duration::from_secs(5)).expect("valid fixture shutdown timeout"),
        )
        .http_server(HttpServerSpec::new(http_server_config(), http_router))
        .grpc_server(GrpcServerSpec::with_routes(
            TaskName::new("second-fixture-grpc").expect("valid fixture gRPC task name"),
            grpc_server_config(),
            grpc_routes,
        ))
        .startup_check(startup_check)
        .background_task(noop_background_task)
        .phase_reporter(phase_reporter)
        .build();

    assert!(result.is_ok());
    assert_eq!(phase_watch.current(), ServerRuntimePhase::Initializing);
}

fn http_server_config_with_socket(socket_addr: SocketAddr) -> HttpServerConfig {
    let request_timeout =
        RequestTimeout::new(Duration::from_secs(5)).expect("valid fixture request timeout");
    let body_limit = RequestBodyLimitBytes::new(1024 * 1024).expect("valid fixture body limit");

    HttpServerConfig::new(
        BindAddress::new(socket_addr).expect("valid fixture socket address"),
        CorsConfig::no_cors(),
        TimeoutConfig::new(request_timeout),
        BodyLimitConfig::new(body_limit),
    )
}
