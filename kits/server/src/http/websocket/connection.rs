// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::future::Future;
use std::error::Error as StdError;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures_util::SinkExt;
use tokio::sync::mpsc;
use tokio::time::{Instant, MissedTickBehavior, interval_at, timeout};
use tracing::{debug, warn};

use crate::http::JsonErrorResponse;
use crate::observability::{RuntimeQueueLabel, record_runtime_queue_saturation};
use crate::task::ShutdownToken;

use super::WebSocketMessage;
use super::error::{
    WebSocketCloseReason, WebSocketConnectionLimitError, WebSocketSendError,
    WebSocketSendErrorReason,
};
use super::heartbeat::WebSocketHeartbeatConfig;
use super::limits::WebSocketLimits;
use super::metrics::{
    WebSocketConnectionErrorLabel, record_websocket_connection_closed,
    record_websocket_connection_error, record_websocket_connection_opened,
    record_websocket_connection_timeout,
};
use super::shutdown::{WebSocketShutdownConfig, gracefully_close_websocket};

static EMPTY_PING_PAYLOAD: Bytes = Bytes::new();
const OUTBOUND_ENQUEUE_TIMEOUT: Duration = Duration::from_secs(5);
const SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const MESSAGE_HANDLER_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONNECTION_AGE: Duration = Duration::from_secs(3_600);

#[path = "connection/handler_timeout.rs"]
mod handler_timeout;
#[path = "connection/handle_application_message.rs"]
mod message_handler;
#[path = "connection/model.rs"]
mod model;
#[path = "connection/protocol_close.rs"]
mod protocol_close;

pub use handler_timeout::{WebSocketHandlerTimeoutError, WebSocketHandlerTimeoutErrorReason};
use message_handler::{ApplicationMessageInput, handle_application_message};
use model::{ApplicationMessageOutcome, OutboundWebSocketEvent};
pub use model::{
    ConnectionId, NoopWebSocketConnectionHooks, WebSocketApplicationMessage,
    WebSocketApplicationMessageOwned, WebSocketConnectionContext, WebSocketConnectionHooks,
    WebSocketConnectionLimiter, WebSocketConnectionOutcome, WebSocketConnectionPermit,
    WebSocketHandlerAction,
};
use protocol_close::websocket_protocol_close_reason;

/// Runtime-owned infrastructure configuration for one managed WebSocket
/// connection.
///
/// Grouping these values keeps the public WebSocket API explicit and avoids
/// spreading connection lifecycle concerns across long argument lists. The
/// runtime is intended to drive one accepted connection inside one Tokio task,
/// rather than spawning detached helper tasks per connection concern.
#[derive(Clone)]
pub struct WebSocketConnectionRuntime {
    limits: WebSocketLimits,
    heartbeat: WebSocketHeartbeatConfig,
    shutdown_config: WebSocketShutdownConfig,
    shutdown: ShutdownToken,
    hooks: Arc<dyn WebSocketConnectionHooks>,
    connection_limiter: WebSocketConnectionLimiter,
    handler_timeout: Duration,
}

impl WebSocketConnectionRuntime {
    /// Creates one managed WebSocket runtime configuration.
    ///
    /// Pass a shared limiter explicitly so all runtimes for the same public route
    /// enforce a single connection cap together.
    pub fn new(
        limits: WebSocketLimits,
        heartbeat: WebSocketHeartbeatConfig,
        shutdown_config: WebSocketShutdownConfig,
        shutdown: ShutdownToken,
        hooks: Arc<dyn WebSocketConnectionHooks>,
        connection_limiter: WebSocketConnectionLimiter,
    ) -> Self {
        Self {
            limits,
            heartbeat,
            shutdown_config,
            shutdown,
            hooks,
            connection_limiter,
            handler_timeout: MESSAGE_HANDLER_TIMEOUT,
        }
    }

    /// Returns the validated connection limits.
    pub const fn limits(&self) -> WebSocketLimits {
        self.limits
    }

    /// Returns the validated heartbeat configuration.
    pub const fn heartbeat(&self) -> WebSocketHeartbeatConfig {
        self.heartbeat
    }

    /// Returns the validated graceful-close configuration.
    pub const fn shutdown_config(&self) -> WebSocketShutdownConfig {
        self.shutdown_config
    }

    /// Returns a cloneable shutdown token for this runtime.
    pub fn shutdown_token(&self) -> ShutdownToken {
        self.shutdown.clone()
    }

    /// Returns the lifecycle hooks for this runtime.
    pub fn hooks(&self) -> Arc<dyn WebSocketConnectionHooks> {
        Arc::clone(&self.hooks)
    }

    fn try_acquire_connection_permit(
        &self,
        source_ip: Option<std::net::IpAddr>,
    ) -> Result<WebSocketConnectionPermit, WebSocketConnectionLimitError> {
        self.connection_limiter.try_acquire_for_source(source_ip)
    }
}

/// Outbound handle for a managed WebSocket connection.
///
/// The handle feeds a bounded queue owned by the connection runtime. This
/// keeps backpressure explicit and prevents services from building unbounded
/// message buffers when a client stops reading.
#[derive(Clone)]
pub struct WebSocketConnectionHandle {
    connection_id: ConnectionId,
    sender: mpsc::Sender<OutboundWebSocketEvent>,
}

impl WebSocketConnectionHandle {
    /// Returns the connection identifier associated with this handle.
    pub const fn connection_id(&self) -> ConnectionId {
        self.connection_id
    }

    /// Enqueues one outbound WebSocket message with async backpressure.
    pub async fn send_message(&self, message: WebSocketMessage) -> Result<(), WebSocketSendError> {
        tokio::time::timeout(
            OUTBOUND_ENQUEUE_TIMEOUT,
            self.sender.send(OutboundWebSocketEvent::Message(message)),
        )
        .await
        .map_err(|_| WebSocketSendError::new(WebSocketSendErrorReason::OutboundQueueFull))?
        .map_err(|_| WebSocketSendError::new(WebSocketSendErrorReason::ConnectionClosed))
    }

    /// Attempts to enqueue one outbound WebSocket message without waiting.
    pub fn try_send_message(&self, message: WebSocketMessage) -> Result<(), WebSocketSendError> {
        self.sender
            .try_send(OutboundWebSocketEvent::Message(message))
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => {
                    record_runtime_queue_saturation(RuntimeQueueLabel::WebSocketOutbound);
                    WebSocketSendError::new(WebSocketSendErrorReason::OutboundQueueFull)
                }
                mpsc::error::TrySendError::Closed(_) => {
                    WebSocketSendError::new(WebSocketSendErrorReason::ConnectionClosed)
                }
            })
    }

    /// Requests graceful close of the connection.
    pub async fn close(&self, reason: WebSocketCloseReason) -> Result<(), WebSocketSendError> {
        tokio::time::timeout(
            OUTBOUND_ENQUEUE_TIMEOUT,
            self.sender.send(OutboundWebSocketEvent::Close(reason)),
        )
        .await
        .map_err(|_| WebSocketSendError::new(WebSocketSendErrorReason::OutboundQueueFull))?
        .map_err(|_| WebSocketSendError::new(WebSocketSendErrorReason::ConnectionClosed))
    }
}

/// Async message handler for one managed WebSocket connection.
///
/// Handlers are owned by a single accepted connection and are invoked with
/// `&mut self`, so they only need `Send` rather than `Sync`. The trait uses a
/// generic associated future instead of `async_trait`; this keeps handler
/// errors strongly typed and avoids forcing boxed futures on every inbound
/// message. It is therefore intended for static dispatch by the WebSocket
/// runtime, not for direct trait-object storage.
///
/// Handler futures must be cancellation-safe. During server-runtime shutdown the
/// runtime may drop an in-progress handler future and close the socket rather
/// than waiting indefinitely.
pub trait WebSocketMessageHandler: Send + 'static {
    /// Typed handler failure. The runtime never logs the error value directly.
    type Error: StdError + Send + Sync + 'static;
    /// Future returned by one inbound-message handling call.
    type HandleFuture<'a>: Future<Output = Result<WebSocketHandlerAction, Self::Error>> + Send + 'a
    where
        Self: 'a;

    /// Handles one text or binary message.
    fn on_message<'a>(
        &'a mut self,
        context: WebSocketConnectionContext,
        message: WebSocketApplicationMessage,
        outbound: &'a WebSocketConnectionHandle,
    ) -> Self::HandleFuture<'a>;
}

/// Applies safe message/frame limits to an Axum WebSocket upgrade.
pub fn configure_websocket_upgrade(
    upgrade: WebSocketUpgrade,
    limits: WebSocketLimits,
) -> WebSocketUpgrade {
    upgrade
        .max_message_size(limits.max_message_size().as_usize())
        .max_frame_size(limits.max_frame_size().as_usize())
}

/// Creates a WebSocket upgrade response backed by the managed connection
/// runtime.
pub fn websocket_upgrade_response<Handler>(
    upgrade: WebSocketUpgrade,
    context: WebSocketConnectionContext,
    runtime: WebSocketConnectionRuntime,
    handler: Handler,
) -> Response
where
    Handler: WebSocketMessageHandler,
{
    let connection_permit = match runtime.try_acquire_connection_permit(context.source_ip()) {
        Ok(permit) => permit,
        Err(_) => {
            return JsonErrorResponse::service_unavailable().into_response();
        }
    };

    configure_websocket_upgrade(upgrade, runtime.limits())
        .on_failed_upgrade(|_| {
            warn!(event = "websocket_upgrade_failed");
        })
        .on_upgrade(move |socket| async move {
            connection_permit.keep_alive();
            let _ = run_websocket_connection(socket, context, runtime, handler).await;
        })
}

/// Runs one managed WebSocket connection to completion.
///
/// The runtime is single-tasked and cancellation-safe:
/// - outbound messages flow through a bounded queue
/// - heartbeat and idle timeout are enforced inside the same select loop
/// - shutdown requests trigger an intentional close handshake
/// - no message contents or tokens are logged
pub(crate) async fn run_websocket_connection<Handler>(
    mut socket: WebSocket,
    context: WebSocketConnectionContext,
    runtime: WebSocketConnectionRuntime,
    mut handler: Handler,
) -> WebSocketConnectionOutcome
where
    Handler: WebSocketMessageHandler,
{
    let mut shutdown = runtime.shutdown_token();
    let hooks = runtime.hooks();
    let limits = runtime.limits();
    let heartbeat = runtime.heartbeat();
    let shutdown_config = runtime.shutdown_config();
    let (sender, mut receiver) = mpsc::channel(limits.outbound_queue_capacity().as_usize());
    let outbound = WebSocketConnectionHandle {
        connection_id: context.connection_id(),
        sender,
    };
    let first_heartbeat_at = Instant::now() + heartbeat.heartbeat_interval().as_duration();
    let mut heartbeat_ticks = interval_at(
        first_heartbeat_at,
        heartbeat.heartbeat_interval().as_duration(),
    );
    heartbeat_ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_inbound_activity = Instant::now();
    let maximum_age_at = Instant::now() + MAX_CONNECTION_AGE;

    record_websocket_connection_opened();
    hooks.on_open(context);
    debug!(
        connection_id = %context.connection_id(),
        event = "websocket_connection_opened"
    );

    let outcome = loop {
        let idle_sleep = tokio::time::sleep_until(
            last_inbound_activity + heartbeat.idle_timeout().as_duration(),
        );
        tokio::pin!(idle_sleep);
        let maximum_age_sleep = tokio::time::sleep_until(maximum_age_at);
        tokio::pin!(maximum_age_sleep);

        tokio::select! {
            shutdown_reason = shutdown.cancelled() => {
                debug!(
                    connection_id = %context.connection_id(),
                    event = "websocket_connection_shutdown_requested",
                    shutdown_reason = ?shutdown_reason
                );
                gracefully_close_websocket(
                    &mut socket,
                    WebSocketCloseReason::ServerShutdown,
                    shutdown_config,
                ).await;
                break WebSocketConnectionOutcome::ServerClosed(WebSocketCloseReason::ServerShutdown);
            }
            _ = heartbeat_ticks.tick() => {
                if !matches!(
                    timeout(SOCKET_WRITE_TIMEOUT, socket.send(Message::Ping(EMPTY_PING_PAYLOAD.clone()))).await,
                    Ok(Ok(()))
                ) {
                    record_websocket_connection_error(WebSocketConnectionErrorLabel::TransportError);
                    break WebSocketConnectionOutcome::TransportError;
                }
            }
            _ = &mut idle_sleep => {
                record_websocket_connection_timeout();
                gracefully_close_websocket(
                    &mut socket,
                    WebSocketCloseReason::IdleTimeout,
                    shutdown_config,
                ).await;
                break WebSocketConnectionOutcome::ServerClosed(WebSocketCloseReason::IdleTimeout);
            }
            _ = &mut maximum_age_sleep => {
                gracefully_close_websocket(
                    &mut socket,
                    WebSocketCloseReason::GoingAway,
                    shutdown_config,
                ).await;
                break WebSocketConnectionOutcome::ServerClosed(WebSocketCloseReason::GoingAway);
            }
            maybe_outbound = receiver.recv() => {
                match maybe_outbound {
                    Some(OutboundWebSocketEvent::Message(message)) => {
                        if !matches!(timeout(SOCKET_WRITE_TIMEOUT, socket.send(message)).await, Ok(Ok(()))) {
                            record_websocket_connection_error(WebSocketConnectionErrorLabel::TransportError);
                            break WebSocketConnectionOutcome::TransportError;
                        }
                    }
                    Some(OutboundWebSocketEvent::Close(reason)) => {
                        gracefully_close_websocket(&mut socket, reason, shutdown_config).await;
                        break WebSocketConnectionOutcome::ServerClosed(reason);
                    }
                    None => {
                        gracefully_close_websocket(
                            &mut socket,
                            WebSocketCloseReason::NormalClosure,
                            shutdown_config,
                        ).await;
                        break WebSocketConnectionOutcome::ServerClosed(WebSocketCloseReason::NormalClosure);
                    }
                }
            }
            inbound = socket.recv() => {
                match inbound {
                    Some(Ok(Message::Text(text))) => {
                        last_inbound_activity = Instant::now();
                        match handle_application_message(
                            &mut handler,
                            ApplicationMessageInput::new(context, WebSocketApplicationMessage::Text(text)),
                            &outbound,
                            &mut receiver,
                            &mut socket,
                            &mut shutdown,
                            runtime.handler_timeout,
                        ).await {
                            ApplicationMessageOutcome::Continue => {}
                            ApplicationMessageOutcome::Close(reason) => {
                                gracefully_close_websocket(&mut socket, reason, shutdown_config).await;
                                break WebSocketConnectionOutcome::ServerClosed(reason);
                            }
                            ApplicationMessageOutcome::HandlerError => {
                                record_websocket_connection_error(WebSocketConnectionErrorLabel::HandlerError);
                                gracefully_close_websocket(
                                    &mut socket,
                                    WebSocketCloseReason::InternalError,
                                    shutdown_config,
                                ).await;
                                break WebSocketConnectionOutcome::HandlerError;
                            }
                            ApplicationMessageOutcome::HandlerTimeout => {
                                record_websocket_connection_timeout();
                                gracefully_close_websocket(&mut socket, WebSocketCloseReason::InternalError, shutdown_config).await;
                                break WebSocketConnectionOutcome::HandlerError;
                            }
                            ApplicationMessageOutcome::TransportError => {
                                record_websocket_connection_error(WebSocketConnectionErrorLabel::TransportError);
                                break WebSocketConnectionOutcome::TransportError;
                            }
                            ApplicationMessageOutcome::ShutdownRequested => {
                                gracefully_close_websocket(
                                    &mut socket,
                                    WebSocketCloseReason::ServerShutdown,
                                    shutdown_config,
                                ).await;
                                break WebSocketConnectionOutcome::ServerClosed(WebSocketCloseReason::ServerShutdown);
                            }
                        }
                    }
                    Some(Ok(Message::Binary(binary))) => {
                        last_inbound_activity = Instant::now();
                        match handle_application_message(
                            &mut handler,
                            ApplicationMessageInput::new(context, WebSocketApplicationMessage::Binary(binary)),
                            &outbound,
                            &mut receiver,
                            &mut socket,
                            &mut shutdown,
                            runtime.handler_timeout,
                        ).await {
                            ApplicationMessageOutcome::Continue => {}
                            ApplicationMessageOutcome::Close(reason) => {
                                gracefully_close_websocket(&mut socket, reason, shutdown_config).await;
                                break WebSocketConnectionOutcome::ServerClosed(reason);
                            }
                            ApplicationMessageOutcome::HandlerError => {
                                record_websocket_connection_error(WebSocketConnectionErrorLabel::HandlerError);
                                gracefully_close_websocket(
                                    &mut socket,
                                    WebSocketCloseReason::InternalError,
                                    shutdown_config,
                                ).await;
                                break WebSocketConnectionOutcome::HandlerError;
                            }
                            ApplicationMessageOutcome::HandlerTimeout => {
                                record_websocket_connection_timeout();
                                gracefully_close_websocket(&mut socket, WebSocketCloseReason::InternalError, shutdown_config).await;
                                break WebSocketConnectionOutcome::HandlerError;
                            }
                            ApplicationMessageOutcome::TransportError => {
                                record_websocket_connection_error(WebSocketConnectionErrorLabel::TransportError);
                                break WebSocketConnectionOutcome::TransportError;
                            }
                            ApplicationMessageOutcome::ShutdownRequested => {
                                gracefully_close_websocket(
                                    &mut socket,
                                    WebSocketCloseReason::ServerShutdown,
                                    shutdown_config,
                                ).await;
                                break WebSocketConnectionOutcome::ServerClosed(WebSocketCloseReason::ServerShutdown);
                            }
                        }
                    }
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                        last_inbound_activity = Instant::now();
                    }
                    Some(Ok(Message::Close(_))) => {
                        // Tungstenite queues the mandatory close reply while
                        // reading the peer frame. Flush that queued reply;
                        // sending a second Close here can fail as AlreadyClosed.
                        let _ = timeout(SOCKET_WRITE_TIMEOUT, socket.flush()).await;
                        break WebSocketConnectionOutcome::PeerClosed;
                    }
                    None => {
                        break WebSocketConnectionOutcome::PeerClosed;
                    }
                    Some(Err(error)) => {
                        record_websocket_connection_error(WebSocketConnectionErrorLabel::TransportError);
                        if let Some(reason) = websocket_protocol_close_reason(&error) {
                            gracefully_close_websocket(&mut socket, reason, shutdown_config).await;
                        }
                        break WebSocketConnectionOutcome::TransportError;
                    }
                }
            }
        }
    };

    record_websocket_connection_closed(outcome);
    hooks.on_close(context, outcome);
    debug!(
        connection_id = %context.connection_id(),
        event = "websocket_connection_closed",
        outcome = outcome.metric_label().as_str()
    );

    outcome
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;
