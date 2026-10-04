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
use tower::Service;
use tracing::{debug, trace};

use super::super::connection_guard::{BoundedTcpListener, BoundedTcpStream};
use crate::config::HttpHeaderLimitConfig;
use crate::http::HttpListenerName;
use crate::task::{ShutdownToken, TaskExecutionError, TaskExecutionErrorKind};

const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(5);
const H2_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);
const H2_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(10);
const H2_MAX_CONCURRENT_STREAMS: u32 = 128;
const HTTP1_REQUEST_LINE_ALLOWANCE_BYTES: usize = 8 * 1024;

pub(crate) async fn serve_http(
    listener: TcpListener,
    router: Router,
    listener_name: HttpListenerName,
    header_limits: HttpHeaderLimitConfig,
    mut shutdown: ShutdownToken,
) -> Result<(), TaskExecutionError> {
    let mut listener = BoundedTcpListener::new(listener);
    let mut connections = JoinSet::new();
    let listener_name = listener_name.as_str().to_owned();
    let max_headers = header_limits
        .max_header_count()
        .as_usize()
        .checked_add(1)
        .ok_or_else(internal_task_error)?;
    let max_header_buffer = header_limits
        .max_header_bytes()
        .as_usize()
        .checked_add(HTTP1_REQUEST_LINE_ALLOWANCE_BYTES)
        .ok_or_else(internal_task_error)?;
    let max_h2_headers = u32::try_from(header_limits.max_header_bytes().as_usize())
        .map_err(|_| internal_task_error())?;

    loop {
        tokio::select! {
            biased;
            reason = shutdown.cancelled() => {
                debug!(listener_name, ?reason, "http_listener_draining");
                break;
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if result.is_some_and(|outcome| outcome.is_err()) {
                    return Err(internal_task_error());
                }
            }
            (stream, peer) = listener.accept() => {
                connections.spawn(serve_connection(
                    stream,
                    peer,
                    router.clone(),
                    shutdown.clone(),
                    max_headers,
                    max_header_buffer,
                    max_h2_headers,
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
            return Err(internal_task_error());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn serve_connection(
    stream: BoundedTcpStream,
    peer: SocketAddr,
    router: Router,
    mut shutdown: ShutdownToken,
    max_headers: usize,
    max_header_buffer: usize,
    max_h2_headers: u32,
) {
    let mut builder = Builder::new(TokioExecutor::new());
    builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(HEADER_READ_TIMEOUT)
        .max_headers(max_headers)
        .max_buf_size(max_header_buffer);
    builder
        .http2()
        .timer(TokioTimer::new())
        .max_header_list_size(max_h2_headers)
        .max_concurrent_streams(H2_MAX_CONCURRENT_STREAMS)
        .keep_alive_interval(H2_KEEPALIVE_INTERVAL)
        .keep_alive_timeout(H2_KEEPALIVE_TIMEOUT)
        .enable_connect_protocol();

    let first_request = stream.first_request_tracker();
    let service = tower::service_fn(move |request: Request<Incoming>| {
        first_request.mark_seen();
        let mut router = router.clone();
        async move {
            let mut request = request.map(Body::new);
            request.extensions_mut().insert(ConnectInfo(peer));
            router.call(request).await
        }
    });
    let hyper_service = TowerToHyperService::new(service);
    let mut connection =
        Box::pin(builder.serve_connection_with_upgrades(TokioIo::new(stream), hyper_service));

    tokio::select! {
        result = &mut connection => {
            if result.is_err() {
                trace!("http_connection_closed_after_transport_error");
            }
        }
        _ = shutdown.cancelled() => {
            connection.as_mut().graceful_shutdown();
            if connection.await.is_err() {
                trace!("http_connection_closed_during_drain");
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
