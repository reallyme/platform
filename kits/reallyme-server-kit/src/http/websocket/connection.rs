// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use core::future::Future;
use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;

use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use tokio::time::{Instant, MissedTickBehavior, interval_at};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::config::RuntimeConcurrencyLimit;
use crate::http::JsonErrorResponse;
use crate::observability::{RuntimeQueueLabel, record_runtime_queue_saturation};
use crate::task::ShutdownToken;
use crate::transport::{RequestId, TraceId};

use super::WebSocketMessage;
use super::error::{
    WebSocketCloseReason, WebSocketConnectionLimitError, WebSocketConnectionLimitErrorReason,
    WebSocketSendError, WebSocketSendErrorReason,
};
use super::heartbeat::WebSocketHeartbeatConfig;
use super::limits::WebSocketLimits;
use super::metrics::{
    WebSocketConnectionErrorLabel, WebSocketConnectionOutcomeLabel,
    record_websocket_connection_closed, record_websocket_connection_error,
    record_websocket_connection_opened, record_websocket_connection_timeout,
};
use super::shutdown::{WebSocketShutdownConfig, gracefully_close_websocket};

static EMPTY_PING_PAYLOAD: Bytes = Bytes::new();

enum OutboundWebSocketEvent {
    Message(WebSocketMessage),
    Close(WebSocketCloseReason),
}

/// Shared limiter for active WebSocket connections.
///
/// Each route should clone one limiter into all accepted connections so active
/// connection count is bounded across that route. The permit is held for the
/// full connection runtime and dropped automatically when the connection ends.
#[derive(Debug, Clone)]
pub struct WebSocketConnectionLimiter {
    semaphore: Arc<Semaphore>,
}

impl WebSocketConnectionLimiter {
    /// Creates a limiter from a validated runtime concurrency limit.
    pub fn new(limit: RuntimeConcurrencyLimit) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(limit.as_usize())),
        }
    }

    /// Attempts to reserve capacity for one active WebSocket connection.
    pub fn try_acquire(&self) -> Result<WebSocketConnectionPermit, WebSocketConnectionLimitError> {
        Arc::clone(&self.semaphore)
            .try_acquire_owned()
            .map(WebSocketConnectionPermit)
            .map_err(|_| {
                WebSocketConnectionLimitError::new(
                    WebSocketConnectionLimitErrorReason::TooManyActiveConnections,
                )
            })
    }
}

/// Permit held while one WebSocket connection is active.
#[derive(Debug)]
pub struct WebSocketConnectionPermit(OwnedSemaphorePermit);

impl WebSocketConnectionPermit {
    fn keep_alive(&self) {
        let _permit = &self.0;
    }
}

/// Typed identifier for one accepted WebSocket connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(Uuid);

impl ConnectionId {
    /// Creates a fresh connection identifier.
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the raw UUID value.
    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0.hyphenated())
    }
}

/// Stable connection context attached to one WebSocket session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebSocketConnectionContext {
    connection_id: ConnectionId,
    request_id: Option<RequestId>,
    trace_id: Option<TraceId>,
}

impl WebSocketConnectionContext {
    /// Creates a fresh connection context.
    pub fn new(request_id: Option<RequestId>, trace_id: Option<TraceId>) -> Self {
        Self {
            connection_id: ConnectionId::generate(),
            request_id,
            trace_id,
        }
    }

    /// Returns the typed connection identifier.
    pub const fn connection_id(self) -> ConnectionId {
        self.connection_id
    }

    /// Returns the correlated request identifier, if any.
    pub const fn request_id(self) -> Option<RequestId> {
        self.request_id
    }

    /// Returns the correlated trace identifier, if any.
    pub const fn trace_id(self) -> Option<TraceId> {
        self.trace_id
    }
}

/// Application-level inbound WebSocket message delivered to a service handler.
///
/// Ping, pong, and close control frames are handled by the infrastructure
/// runtime and are not surfaced to service handlers.
#[derive(Clone, PartialEq, Eq)]
pub enum WebSocketApplicationMessage {
    /// UTF-8 text payload.
    Text(Utf8Bytes),
    /// Binary payload.
    Binary(Bytes),
}

impl fmt::Debug for WebSocketApplicationMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => formatter
                .debug_struct("Text")
                .field("byte_len", &text.len())
                .field("payload", &"<redacted>")
                .finish(),
            Self::Binary(bytes) => formatter
                .debug_struct("Binary")
                .field("byte_len", &bytes.len())
                .field("payload", &"<redacted>")
                .finish(),
        }
    }
}

impl WebSocketApplicationMessage {
    /// Converts the message payload into owned buffers for handlers that need
    /// owned values.
    pub fn into_owned(self) -> WebSocketApplicationMessageOwned {
        match self {
            Self::Text(text) => WebSocketApplicationMessageOwned::Text(text.to_string()),
            Self::Binary(binary) => WebSocketApplicationMessageOwned::Binary(binary.to_vec()),
        }
    }
}

/// Owned representation for handlers that need concrete, detached payload buffers.
#[derive(Clone, PartialEq, Eq)]
pub enum WebSocketApplicationMessageOwned {
    /// UTF-8 text payload.
    Text(String),
    /// Binary payload.
    Binary(Vec<u8>),
}

impl fmt::Debug for WebSocketApplicationMessageOwned {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => formatter
                .debug_struct("Text")
                .field("byte_len", &text.len())
                .field("payload", &"<redacted>")
                .finish(),
            Self::Binary(binary) => formatter
                .debug_struct("Binary")
                .field("byte_len", &binary.len())
                .field("payload", &"<redacted>")
                .finish(),
        }
    }
}

impl WebSocketApplicationMessageOwned {
    /// Returns the UTF-8 text payload, if present.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text.as_str()),
            Self::Binary(_) => None,
        }
    }

    /// Returns the binary payload, if present.
    pub fn binary(&self) -> Option<&[u8]> {
        match self {
            Self::Text(_) => None,
            Self::Binary(binary) => Some(binary.as_slice()),
        }
    }
}

/// Handler-directed control flow after processing one inbound message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketHandlerAction {
    /// Keep the connection open.
    Continue,
    /// Close the connection with the provided close reason.
    Close(WebSocketCloseReason),
}

/// Final low-cardinality outcome of a WebSocket connection runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketConnectionOutcome {
    /// The peer closed the socket.
    PeerClosed,
    /// The server closed the socket intentionally.
    ServerClosed(WebSocketCloseReason),
    /// The handler failed internally and the socket was closed safely.
    HandlerError,
    /// The transport failed unexpectedly.
    TransportError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplicationMessageOutcome {
    Continue,
    Close(WebSocketCloseReason),
    HandlerError,
    ShutdownRequested,
}

impl WebSocketConnectionOutcome {
    /// Returns the low-cardinality metric label for this outcome.
    pub const fn metric_label(self) -> WebSocketConnectionOutcomeLabel {
        match self {
            Self::PeerClosed => WebSocketConnectionOutcomeLabel::PeerClosed,
            Self::ServerClosed(_) => WebSocketConnectionOutcomeLabel::ServerClosed,
            Self::HandlerError => WebSocketConnectionOutcomeLabel::HandlerError,
            Self::TransportError => WebSocketConnectionOutcomeLabel::TransportError,
        }
    }
}

/// Lifecycle hook surface for connection open/close events.
///
/// Hooks are synchronous on purpose. They should perform small in-memory side
/// effects only and must not block or perform network I/O.
pub trait WebSocketConnectionHooks: Send + Sync + 'static {
    /// Called when a connection becomes active.
    fn on_open(&self, _context: WebSocketConnectionContext) {}

    /// Called when a connection finishes with a final outcome.
    fn on_close(&self, _context: WebSocketConnectionContext, _outcome: WebSocketConnectionOutcome) {
    }
}

/// No-op lifecycle hooks for callers that do not need notifications.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopWebSocketConnectionHooks;

impl WebSocketConnectionHooks for NoopWebSocketConnectionHooks {}

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
    ) -> Result<WebSocketConnectionPermit, WebSocketConnectionLimitError> {
        self.connection_limiter.try_acquire()
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
        self.sender
            .send(OutboundWebSocketEvent::Message(message))
            .await
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
        self.sender
            .send(OutboundWebSocketEvent::Close(reason))
            .await
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
    let connection_permit = match runtime.try_acquire_connection_permit() {
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
pub async fn run_websocket_connection<Handler>(
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
                if socket.send(Message::Ping(EMPTY_PING_PAYLOAD.clone())).await.is_err() {
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
            maybe_outbound = receiver.recv() => {
                match maybe_outbound {
                    Some(OutboundWebSocketEvent::Message(message)) => {
                        if socket.send(message).await.is_err() {
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
                            context,
                            WebSocketApplicationMessage::Text(text),
                            &outbound,
                            &mut shutdown,
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
                            context,
                            WebSocketApplicationMessage::Binary(binary),
                            &outbound,
                            &mut shutdown,
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
                    Some(Ok(Message::Close(_))) | None => {
                        break WebSocketConnectionOutcome::PeerClosed;
                    }
                    Some(Err(_)) => {
                        record_websocket_connection_error(WebSocketConnectionErrorLabel::TransportError);
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

async fn handle_application_message<Handler>(
    handler: &mut Handler,
    context: WebSocketConnectionContext,
    message: WebSocketApplicationMessage,
    outbound: &WebSocketConnectionHandle,
    shutdown: &mut ShutdownToken,
) -> ApplicationMessageOutcome
where
    Handler: WebSocketMessageHandler,
{
    tokio::select! {
        result = handler.on_message(context, message, outbound) => {
            match result {
                Ok(WebSocketHandlerAction::Continue) => ApplicationMessageOutcome::Continue,
                Ok(WebSocketHandlerAction::Close(reason)) => ApplicationMessageOutcome::Close(reason),
                Err(_) => ApplicationMessageOutcome::HandlerError,
            }
        }
        _ = shutdown.cancelled() => ApplicationMessageOutcome::ShutdownRequested,
    }
}

#[cfg(test)]
fn make_outbound_channel(
    connection_id: ConnectionId,
    capacity: usize,
) -> (
    WebSocketConnectionHandle,
    mpsc::Receiver<OutboundWebSocketEvent>,
) {
    let (sender, receiver) = mpsc::channel(capacity);
    (
        WebSocketConnectionHandle {
            connection_id,
            sender,
        },
        receiver,
    )
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;
