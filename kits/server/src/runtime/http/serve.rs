// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Owned HTTP connection tasks and transport parser limits.

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::Request;
use axum::serve::Listener;
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use hyper_util::server::conn::auto::Builder;
use hyper_util::service::TowerToHyperService;
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio::time::sleep_until;
use tower::Service;
use tracing::{debug, trace, warn};

#[path = "activity.rs"]
mod activity;
use activity::{ConnectionActivity, TrackedResponseBody};
#[path = "write_progress.rs"]
mod write_progress;
use write_progress::WriteProgressIo;

use super::super::connection_guard::{BoundedTcpListener, BoundedTcpStream};
use crate::config::{
    ConnectionLimitConfig, HttpHeaderLimitConfig, HttpServerConfig, TrustedProxyHeaders,
};
use crate::http::HttpListenerName;
use crate::task::{ShutdownToken, TaskExecutionError, TaskExecutionErrorKind};

const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(5);
const H2_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);
const H2_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(10);
const H2_MAX_CONCURRENT_STREAMS: u32 = 128;
const HTTP1_REQUEST_LINE_ALLOWANCE_BYTES: usize = 8 * 1024;
const MAX_HTTP_CONNECTION_AGE: Duration = Duration::from_secs(600);
const CONNECTION_DRAIN_GRACE: Duration = Duration::from_secs(30);
const HTTP_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const HTTP_WRITE_STALL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
struct ConnectionServeSettings {
    max_headers: usize,
    max_header_buffer: usize,
    max_h2_headers: u32,
    max_connection_age: Duration,
    connection_drain_grace: Duration,
    idle_timeout: Duration,
    write_stall_timeout: Duration,
}

pub(crate) struct HttpServePolicy {
    listener_name: HttpListenerName,
    header_limits: HttpHeaderLimitConfig,
    connection_limits: ConnectionLimitConfig,
    trusted_proxies: TrustedProxyHeaders,
    max_connection_age: Duration,
    connection_drain_grace: Duration,
    idle_timeout: Duration,
    write_stall_timeout: Duration,
}

impl HttpServePolicy {
    pub(crate) fn from_config(listener_name: HttpListenerName, config: &HttpServerConfig) -> Self {
        Self {
            listener_name,
            header_limits: config.security().header_limits(),
            connection_limits: config.connection_limits(),
            trusted_proxies: config.security().trusted_proxy_headers().clone(),
            max_connection_age: MAX_HTTP_CONNECTION_AGE,
            connection_drain_grace: CONNECTION_DRAIN_GRACE,
            idle_timeout: HTTP_IDLE_TIMEOUT,
            write_stall_timeout: HTTP_WRITE_STALL_TIMEOUT,
        }
    }
}

pub(crate) async fn serve_http(
    listener: TcpListener,
    router: Router,
    policy: HttpServePolicy,
    mut shutdown: ShutdownToken,
) -> Result<(), TaskExecutionError> {
    let mut listener = BoundedTcpListener::new(listener, policy.connection_limits)
        .with_trusted_proxies(policy.trusted_proxies);
    let mut connections = JoinSet::new();
    let listener_name = policy.listener_name.as_str().to_owned();
    let max_headers = policy
        .header_limits
        .max_header_count()
        .as_usize()
        .checked_add(1)
        .ok_or_else(internal_task_error)?;
    let max_header_buffer = policy
        .header_limits
        .max_header_bytes()
        .as_usize()
        .checked_add(HTTP1_REQUEST_LINE_ALLOWANCE_BYTES)
        .ok_or_else(internal_task_error)?;
    let max_h2_headers = u32::try_from(policy.header_limits.max_header_bytes().as_usize())
        .map_err(|_| internal_task_error())?;
    let settings = ConnectionServeSettings {
        max_headers,
        max_header_buffer,
        max_h2_headers,
        max_connection_age: policy.max_connection_age,
        connection_drain_grace: policy.connection_drain_grace,
        idle_timeout: policy.idle_timeout,
        write_stall_timeout: policy.write_stall_timeout,
    };

    loop {
        tokio::select! {
            biased;
            reason = shutdown.cancelled() => {
                debug!(listener_name, ?reason, "http_listener_draining");
                break;
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if result.is_some_and(|outcome| outcome.is_err()) {
                    // A response body may panic after the handler returns.
                    // Contain that failure to its connection; the listener is
                    // still healthy and must continue accepting clients.
                    warn!("http_connection_task_failed");
                }
            }
            (stream, peer) = listener.accept() => {
                connections.spawn(serve_connection(
                    stream,
                    peer,
                    router.clone(),
                    shutdown.clone(),
                    settings,
                ));
            }
        }
    }
    drop(listener);

    // The JoinSet owns every connection task. If the supervisor's drain
    // deadline expires and it aborts this listener task, dropping the set
    // aborts the remaining connections before application cleanup proceeds.
    while let Some(result) = connections.join_next().await {
        if result.is_err() {
            warn!("http_connection_task_failed_during_drain");
        }
    }
    Ok(())
}

async fn serve_connection(
    stream: BoundedTcpStream,
    peer: SocketAddr,
    router: Router,
    mut shutdown: ShutdownToken,
    settings: ConnectionServeSettings,
) {
    let mut builder = Builder::new(TokioExecutor::new());
    builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(HEADER_READ_TIMEOUT)
        .max_headers(settings.max_headers)
        .max_buf_size(settings.max_header_buffer);
    builder
        .http2()
        .timer(TokioTimer::new())
        .max_header_list_size(settings.max_h2_headers)
        .max_concurrent_streams(H2_MAX_CONCURRENT_STREAMS)
        .keep_alive_interval(H2_KEEPALIVE_INTERVAL)
        .keep_alive_timeout(H2_KEEPALIVE_TIMEOUT)
        .enable_connect_protocol();

    let first_request = stream.first_request_tracker();
    let (activity, mut activity_rx) = ConnectionActivity::new();
    let service = tower::service_fn(move |request: Request<Incoming>| {
        first_request.mark_seen();
        let mut router = router.clone();
        let request_guard = activity.begin_request();
        async move {
            let mut request = request.map(Body::new);
            request.extensions_mut().insert(ConnectInfo(peer));
            router.call(request).await.map(|response| {
                response.map(|body| Body::new(TrackedResponseBody::new(body, request_guard)))
            })
        }
    });
    let hyper_service = TowerToHyperService::new(service);
    let io = WriteProgressIo::new(stream, settings.write_stall_timeout);
    let mut connection =
        Box::pin(builder.serve_connection_with_upgrades(TokioIo::new(io), hyper_service));
    let mut age_deadline = Box::pin(tokio::time::sleep(settings.max_connection_age));
    let idle_since = activity_rx.borrow().idle_since;
    let mut idle_deadline = Box::pin(sleep_until(idle_since + settings.idle_timeout));

    loop {
        let has_active_requests = activity_rx.borrow().active_requests != 0;
        tokio::select! {
            biased;
            result = &mut connection => {
                if result.is_err() {
                    trace!("http_connection_closed_after_transport_error");
                }
                break;
            }
            _ = shutdown.cancelled() => {
                connection.as_mut().graceful_shutdown();
                if connection.await.is_err() {
                    trace!("http_connection_closed_during_drain");
                }
                break;
            }
            _ = &mut age_deadline => {
                // Hyper sends GOAWAY for HTTP/2 and finishes in-flight streams.
                // Upgraded WebSockets have their own close-frame age policy.
                connection.as_mut().graceful_shutdown();
                if tokio::time::timeout(settings.connection_drain_grace, connection)
                    .await
                    .is_err()
                {
                    trace!("http_connection_drain_grace_elapsed");
                }
                break;
            }
            changed = activity_rx.changed() => {
                if changed.is_ok() {
                    let snapshot = *activity_rx.borrow_and_update();
                    if snapshot.active_requests == 0 {
                        idle_deadline.as_mut().reset(snapshot.idle_since + settings.idle_timeout);
                    }
                }
            }
            _ = &mut idle_deadline, if !has_active_requests => {
                // A completed keep-alive connection consumes capacity without
                // serving work. Retire it with protocol-level shutdown.
                connection.as_mut().graceful_shutdown();
                if tokio::time::timeout(settings.connection_drain_grace, connection)
                    .await
                    .is_err()
                {
                    trace!("http_connection_idle_drain_grace_elapsed");
                }
                break;
            }
        }
    }
}

fn internal_task_error() -> TaskExecutionError {
    TaskExecutionError::new(TaskExecutionErrorKind::Internal)
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
