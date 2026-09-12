// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::net::{TcpListener as StdTcpListener, TcpStream as StdTcpStream};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use axum::Router;
use axum::routing::get;

#[cfg(feature = "tonic-grpc")]
use crate::config::GrpcServerConfig;
use crate::config::{
    BindAddress, BodyLimitConfig, CorsConfig, HttpServerConfig, LogFormat, MetricsIdleTimeout,
    ObservabilityConfig, RequestBodyLimitBytes, RequestTimeout, ServiceEnvironment, TimeoutConfig,
};
use crate::runtime::app::RuntimeAppCleanup;
use crate::runtime::{
    AppName, HttpServerSpec, RuntimeCleanupHook, ServerRuntime, ServerRuntimeError,
};
use crate::startup::{ServerName, TaskName};
use crate::task::{ShutdownTimeout, TaskExecutionError};
use crate::version::BuildInfo;

const STDOUT_LOCK_REGRESSION_ENV: &str = "REALLYME_SERVER_KIT_RUNTIME_STDOUT_LOCK_REGRESSION";
const STDOUT_LOCK_REGRESSION_PORT_ENV: &str =
    "REALLYME_SERVER_KIT_RUNTIME_STDOUT_LOCK_REGRESSION_PORT";
const PHASE_REGRESSION_ENV: &str = "REALLYME_SERVER_KIT_RUNTIME_PHASE_REGRESSION";
const CRITICAL_TASK_REGRESSION_ENV: &str = "REALLYME_SERVER_KIT_RUNTIME_CRITICAL_TASK_REGRESSION";

pub(super) fn observability_config() -> ObservabilityConfig {
    ObservabilityConfig::new(
        ServiceEnvironment::Local,
        LogFormat::PlainText,
        false,
        "reallyme_server_kit=info".to_owned(),
        MetricsIdleTimeout::new(Duration::from_secs(60))
            .expect("valid fixture metrics idle timeout"),
    )
    .expect("valid fixture observability config")
}

#[cfg(feature = "tonic-grpc")]
pub(super) fn http_server_config() -> HttpServerConfig {
    http_server_config_for_port(18_080)
}

pub(super) fn http_server_config_for_port(port: u16) -> HttpServerConfig {
    let request_timeout =
        RequestTimeout::new(Duration::from_secs(5)).expect("valid fixture request timeout");
    let body_limit =
        RequestBodyLimitBytes::new(1024 * 1024).expect("valid fixture request body limit");

    HttpServerConfig::new(
        bind_address(port),
        CorsConfig::no_cors(),
        TimeoutConfig::new(request_timeout),
        BodyLimitConfig::new(body_limit),
    )
}

#[cfg(feature = "tonic-grpc")]
pub(super) fn grpc_server_config() -> GrpcServerConfig {
    GrpcServerConfig::new(bind_address(15_051))
}

pub(super) fn cleanup_entry(
    app_name: &'static str,
    hook_name: &'static str,
    marker: &'static str,
    observed: &Arc<Mutex<Vec<&'static str>>>,
) -> RuntimeAppCleanup {
    let observed = Arc::clone(observed);

    RuntimeAppCleanup {
        app_name: AppName::new(app_name).expect("valid fixture app name"),
        hook: RuntimeCleanupHook::new(
            TaskName::new(hook_name).expect("valid fixture task name"),
            move || async move {
                observed
                    .lock()
                    .expect("test mutex should not be poisoned")
                    .push(marker);
                Ok::<(), TaskExecutionError>(())
            },
        ),
    }
}

pub(super) fn bind_address(port: u16) -> BindAddress {
    let socket_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));

    BindAddress::new(socket_addr).expect("valid fixture bind address")
}

pub(super) fn stdout_lock_regression_runtime(
    port: u16,
) -> Result<ServerRuntime, ServerRuntimeError> {
    let server_name =
        ServerName::new("runtime-stdout-lock-regression").expect("valid fixture server name");
    let router = Router::new().route(
        "/hello",
        get(|| async { "hello from runtime stdout lock regression\n" }),
    );

    ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(observability_config())
        .build_info(BuildInfo::new(server_name))
        .readiness(crate::health::Readiness::new())
        .shutdown_timeout(
            ShutdownTimeout::new(Duration::from_secs(2)).expect("valid fixture shutdown timeout"),
        )
        .http_server(HttpServerSpec::new(
            http_server_config_for_port(port),
            router,
        ))
        .build()
}

pub(super) fn unused_local_port() -> Option<u16> {
    let listener = match StdTcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
        Ok(listener) => listener,
        // Some local sandboxes deny opening TCP listeners. CI must run these
        // tests with socket access so real process/listener behavior stays
        // covered; this branch exists only to keep restricted sandboxes usable.
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("ephemeral test listener should bind: {error}"),
    };

    Some(
        listener
            .local_addr()
            .expect("ephemeral test listener should expose address")
            .port(),
    )
}

pub(super) fn spawn_stdout_lock_regression_worker(port: u16) -> Child {
    let current_exe = std::env::current_exe().expect("current test binary path should resolve");

    Command::new(current_exe)
        .arg("--exact")
        .arg("runtime::server::tests::shutdown::stdout_lock_regression_subprocess_worker")
        .env_clear()
        .env(STDOUT_LOCK_REGRESSION_ENV, "run-runtime")
        .env(STDOUT_LOCK_REGRESSION_PORT_ENV, port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("runtime regression subprocess should launch")
}

pub(super) fn spawn_phase_regression_worker(action: &'static str) -> std::process::Output {
    let current_exe = std::env::current_exe().expect("current test binary path should resolve");

    Command::new(current_exe)
        .arg("--exact")
        .arg("runtime::server::tests::phases::phase_regression_subprocess_worker")
        .env_clear()
        .env(PHASE_REGRESSION_ENV, action)
        .output()
        .expect("phase regression subprocess should launch")
}

pub(super) fn stdout_lock_regression_action() -> Option<std::ffi::OsString> {
    std::env::var_os(STDOUT_LOCK_REGRESSION_ENV)
}

pub(super) fn stdout_lock_regression_port() -> u16 {
    std::env::var(STDOUT_LOCK_REGRESSION_PORT_ENV)
        .expect("worker port should be configured")
        .parse::<u16>()
        .expect("worker port should parse")
}

pub(super) fn phase_regression_action() -> Option<std::ffi::OsString> {
    std::env::var_os(PHASE_REGRESSION_ENV)
}

pub(super) fn spawn_critical_task_regression_worker(action: &'static str) -> std::process::Output {
    let current_exe = std::env::current_exe().expect("current test binary path should resolve");

    Command::new(current_exe)
        .arg("--exact")
        .arg("runtime::server::tests::critical::critical_task_regression_subprocess_worker")
        .env_clear()
        .env(CRITICAL_TASK_REGRESSION_ENV, action)
        .output()
        .expect("critical task regression subprocess should launch")
}

pub(super) fn critical_task_regression_action() -> Option<std::ffi::OsString> {
    std::env::var_os(CRITICAL_TASK_REGRESSION_ENV)
}

pub(super) fn wait_for_http_response(
    port: u16,
    path: &str,
    timeout: Duration,
) -> Result<String, std::io::Error> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;

    while Instant::now() < deadline {
        match http_get(port, path) {
            Ok(response) => return Ok(response),
            Err(error) => last_error = Some(error),
        }

        // Subprocess readiness is observable only through the OS socket here.
        // Keep the poll bounded and low-frequency rather than spinning.
        thread::park_timeout(Duration::from_millis(25));
    }

    Err(last_error
        .unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::TimedOut, "request timed out")))
}

pub(super) fn send_sigterm(child: &mut Child) {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(child.id().to_string())
            .status()
            .expect("kill command should launch");

        assert!(status.success(), "kill -TERM should succeed");
    }

    #[cfg(not(unix))]
    child.kill().expect("child process should terminate");
}

pub(super) fn terminate_child_for_cleanup(child: &mut Child) {
    let _ = child.kill();
}

pub(super) struct ChildOutput {
    pub(super) status: Option<ExitStatus>,
    pub(super) stdout: String,
    pub(super) stderr: String,
}

pub(super) fn wait_for_child_output(mut child: Child, timeout: Duration) -> ChildOutput {
    let deadline = Instant::now() + timeout;
    let mut status = None;

    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(exit_status)) => {
                status = Some(exit_status);
                break;
            }
            Ok(None) => {
                // `try_wait` has no readiness primitive, so this bounded
                // subprocess poll intentionally parks instead of busy-spinning.
                thread::park_timeout(Duration::from_millis(25));
            }
            Err(error) => panic!("failed to wait for child process: {error}"),
        }
    }

    if status.is_none() {
        terminate_child_for_cleanup(&mut child);
        status = child.wait().ok();
    }

    let mut stdout = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        pipe.read_to_string(&mut stdout)
            .expect("child stdout should be readable");
    }

    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        pipe.read_to_string(&mut stderr)
            .expect("child stderr should be readable");
    }

    ChildOutput {
        status,
        stdout,
        stderr,
    }
}

fn http_get(port: u16, path: &str) -> Result<String, std::io::Error> {
    let mut stream = StdTcpStream::connect((Ipv4Addr::LOCALHOST, port))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}
