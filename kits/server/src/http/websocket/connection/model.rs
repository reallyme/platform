// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! WebSocket connection identity, messages, outcomes, hooks, and admission permits.

use std::fmt;
use std::sync::Arc;

use axum::extract::ws::Utf8Bytes;
use bytes::Bytes;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

use crate::config::RuntimeConcurrencyLimit;
use crate::transport::{RequestId, TraceId};

use super::super::WebSocketMessage;
use super::super::error::{
    WebSocketCloseReason, WebSocketConnectionLimitError, WebSocketConnectionLimitErrorReason,
};
use super::super::metrics::WebSocketConnectionOutcomeLabel;

pub(super) enum OutboundWebSocketEvent {
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
    pub(super) fn keep_alive(&self) {
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
pub(super) enum ApplicationMessageOutcome {
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
