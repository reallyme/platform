// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Owned HTTP connection tasks and transport parser limits.

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode};
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
const H2_FLOW_CONTROL_STALL_MULTIPLIER: u32 = 4;
const HTTP2_WEBSOCKET_MAX_AGE: Duration = Duration::from_secs(3_600);
const HTTP1_REQUEST_LINE_ALLOWANCE_BYTES: usize = 8 * 1024;
const LISTENER_ABORT_SETTLE_RESERVE: Duration = Duration::from_millis(100);
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
    shutdown_drain_budget: Duration,
}

impl HttpServePolicy {
    pub(crate) fn from_config(
        listener_name: HttpListenerName,
        config: &HttpServerConfig,
        shutdown_timeout: Duration,
    ) -> Self {
        let timeouts = config.transport_timeouts();
        Self {
            listener_name,
            header_limits: config.security().header_limits(),
            connection_limits: config.connection_limits(),
            trusted_proxies: config.security().trusted_proxy_headers().clone(),
            max_connection_age: timeouts.max_connection_age(),
            connection_drain_grace: timeouts.drain_grace(),
            idle_timeout: timeouts.idle(),
            write_stall_timeout: timeouts.write_stall(),
            // Leave the supervisor time to observe child-task cancellation
            // before it begins application cleanup at the global deadline.
            shutdown_drain_budget: shutdown_timeout
                .saturating_sub(LISTENER_ABORT_SETTLE_RESERVE.min(shutdown_timeout / 2)),
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

    // End overdue connection tasks before the outer task-set deadline. A
    // listener abort at that deadline would otherwise drop its JoinSet and
    // let application cleanup race with asynchronous child cancellation.
    let drain = async {
        while let Some(result) = connections.join_next().await {
            if result.is_err() {
                warn!("http_connection_task_failed_during_drain");
            }
        }
    };
    if tokio::time::timeout(policy.shutdown_drain_budget, drain)
        .await
        .is_err()
    {
        connections.abort_all();
        while connections.join_next().await.is_some() {}
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
    let request_activity = activity.clone();
    let service = tower::service_fn(move |request: Request<Incoming>| {
        first_request.mark_seen();
        let mut router = router.clone();
        let request_guard = request_activity.begin_request();
        let websocket_activity = request_activity.clone();
        let h2_websocket = request.method() == Method::CONNECT
            && request
                .extensions()
                .get::<hyper::ext::Protocol>()
                .is_some_and(|protocol| protocol.as_str() == "websocket");
        async move {
            let mut request = request.map(Body::new);
            request.extensions_mut().insert(ConnectInfo(peer));
            router.call(request).await.map(|response| {
                if h2_websocket && response.status() == StatusCode::OK {
                    websocket_activity.mark_h2_websocket();
                }
                response.map(|body| Body::new(TrackedResponseBody::new(body, request_guard)))
            })
        }
    });
    let hyper_service = TowerToHyperService::new(service);
    let io = WriteProgressIo::new(stream, settings.write_stall_timeout).with_activity(activity);
    let mut connection =
        Box::pin(builder.serve_connection_with_upgrades(TokioIo::new(io), hyper_service));
    let connection_started = tokio::time::Instant::now();
    let mut age_deadline = Box::pin(tokio::time::sleep(settings.max_connection_age));
    // A response body can finish producing bytes before Hyper flushes them to
    // a slow peer. Give queued writes at least the configured stall allowance
    // before treating the connection as an idle keep-alive socket.
    let drain_safe_idle_timeout = settings.idle_timeout.max(settings.write_stall_timeout);
    let idle_since = activity_rx.borrow().idle_since;
    let mut idle_deadline = Box::pin(sleep_until(idle_since + drain_safe_idle_timeout));
    let mut frame_stall_deadline = Box::pin(tokio::time::sleep(Duration::from_secs(86_400)));
    // Flow-control windows can pause a healthy slow reader without a stalled
    // socket write. Give this protocol guard a separate, longer allowance.
    let flow_control_stall_timeout = settings
        .write_stall_timeout
        .saturating_mul(H2_FLOW_CONTROL_STALL_MULTIPLIER);

    loop {
        let snapshot = *activity_rx.borrow();
        let has_active_requests = snapshot.active_requests != 0;
        let unconsumed_frame_at = snapshot.oldest_unconsumed_frame_at;
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
                // The upgrade notification can still be pending when the
                // original age deadline fires on a busy executor. Inspect
                // current activity before retiring an upgraded stream.
                let upgraded_deadline = connection_started
                    + settings.max_connection_age.max(HTTP2_WEBSOCKET_MAX_AGE);
                if activity_rx.borrow().has_h2_websocket
                    && tokio::time::Instant::now() < upgraded_deadline
                {
                    age_deadline.as_mut().reset(upgraded_deadline);
                    continue;
                }
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
                    if snapshot.has_h2_websocket {
                        age_deadline.as_mut().reset(
                            connection_started
                                + settings.max_connection_age.max(HTTP2_WEBSOCKET_MAX_AGE),
                        );
                    }
                    if snapshot.active_requests == 0 {
                        idle_deadline
                            .as_mut()
                            .reset(snapshot.idle_since + drain_safe_idle_timeout);
                    }
                    if let Some(pending_since) = snapshot.oldest_unconsumed_frame_at {
                        frame_stall_deadline
                            .as_mut()
                            .reset(pending_since + flow_control_stall_timeout);
                    }
                }
            }
            _ = &mut frame_stall_deadline,
                if unconsumed_frame_at.is_some()
                    && snapshot.pending_frame_count >= snapshot.active_requests
                    && snapshot.pending_frame_count > 0 => {
                // Hyper owns stream flow control, so the transport cannot
                // reset one stalled stream here. Send GOAWAY and give other
                // in-flight streams their configured drain allowance.
                trace!("http_body_frame_delivery_stalled");
                connection.as_mut().graceful_shutdown();
                if tokio::time::timeout(settings.connection_drain_grace, connection)
                    .await
                    .is_err()
                {
                    trace!("http_flow_control_drain_grace_elapsed");
                }
                break;
            }
            _ = &mut idle_deadline, if !has_active_requests => {
                // A completed keep-alive connection consumes capacity without
                // serving work. GOAWAY prevents an old upgrade marker from
                // reserving this connection for unrelated new requests.
                connection.as_mut().graceful_shutdown();
                let drain = if snapshot.has_h2_websocket {
                    // Hyper owns the upgraded stream. Let it complete within
                    // the same age bound used for a live WebSocket; a closed
                    // upgrade makes the connection future finish promptly.
                    (connection_started
                        + settings.max_connection_age.max(HTTP2_WEBSOCKET_MAX_AGE))
                    .saturating_duration_since(tokio::time::Instant::now())
                } else {
                    settings.connection_drain_grace
                };
                if tokio::time::timeout(drain, connection)
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
