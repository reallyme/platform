// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Owns bounded gRPC connection futures until their graceful drain completes.

use axum::serve::Listener;
use futures_util::{StreamExt, future::BoxFuture, stream::FuturesUnordered};

use crate::runtime::connection_guard::{BoundedTcpListener, BoundedTcpStream};
use crate::task::{ShutdownToken, TaskExecutionError, TaskExecutionErrorKind};

pub(super) async fn serve_connections<F>(
    mut listener: BoundedTcpListener,
    make_connection: F,
    mut shutdown: ShutdownToken,
) -> Result<(), TaskExecutionError>
where
    F: Fn(
        BoundedTcpStream,
        ShutdownToken,
    ) -> BoxFuture<'static, Result<(), tonic::transport::Error>>,
{
    let mut connections: FuturesUnordered<BoxFuture<'static, Result<(), tonic::transport::Error>>> =
        FuturesUnordered::new();
    loop {
        tokio::select! {
            biased;
            _reason = shutdown.cancelled() => {
                // Every connection's signal receives the same shutdown token.
                // Keep polling them until Tonic completes its GOAWAY drain.
                while let Some(result) = connections.next().await {
                    result.map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))?;
                }
                return Ok(());
            }
            result = connections.next(), if !connections.is_empty() => {
                if let Some(result) = result {
                    result.map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))?;
                }
            }
            (stream, _peer) = listener.accept() => {
                connections.push(make_connection(stream, shutdown.clone()));
            }
        }
    }
}
