// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reusable WebSocket infrastructure helpers.
//!
//! This module intentionally stays at the transport/runtime layer. It provides
//! bounded connection primitives, heartbeat/idle helpers, graceful shutdown
//! integration, and typed close semantics without introducing any product
//! protocol, business message, or app-specific realtime contract.

mod connection;
mod error;
mod heartbeat;
mod limits;
mod metrics;
mod shutdown;

pub use axum::extract::ws::{
    CloseFrame, Message as WebSocketMessage, WebSocket, WebSocketUpgrade,
    close_code as websocket_close_code,
};
pub use connection::{
    ConnectionId, NoopWebSocketConnectionHooks, WebSocketApplicationMessage,
    WebSocketConnectionContext, WebSocketConnectionHandle, WebSocketConnectionHooks,
    WebSocketConnectionLimiter, WebSocketConnectionOutcome, WebSocketConnectionPermit,
    WebSocketConnectionRuntime, WebSocketHandlerAction, WebSocketMessageHandler,
    configure_websocket_upgrade, run_websocket_connection, websocket_upgrade_response,
};
pub use error::{
    WebSocketCloseReason, WebSocketConfigError, WebSocketConnectionLimitError,
    WebSocketConnectionLimitErrorReason, WebSocketHeartbeatConfigField, WebSocketLimitField,
    WebSocketSendError, WebSocketSendErrorReason, WebSocketShutdownConfigField,
    WebSocketValidationErrorReason,
};
pub use heartbeat::{
    DEFAULT_HEARTBEAT_INTERVAL, DEFAULT_IDLE_TIMEOUT, HeartbeatInterval, IdleTimeout,
    MAXIMUM_HEARTBEAT_INTERVAL, MAXIMUM_IDLE_TIMEOUT, MINIMUM_HEARTBEAT_INTERVAL,
    MINIMUM_IDLE_TIMEOUT, WebSocketHeartbeatConfig,
};
pub use limits::{
    DEFAULT_INBOUND_FRAME_SIZE_BYTES, DEFAULT_INBOUND_MESSAGE_SIZE_BYTES,
    DEFAULT_OUTBOUND_QUEUE_CAPACITY, InboundFrameSizeBytes, InboundMessageSizeBytes,
    MAX_INBOUND_FRAME_SIZE_BYTES, MAX_INBOUND_MESSAGE_SIZE_BYTES, MAX_OUTBOUND_QUEUE_CAPACITY,
    OutboundQueueCapacity, WebSocketLimits,
};
pub use metrics::{
    WebSocketConnectionErrorLabel, WebSocketConnectionOutcomeLabel, describe_websocket_metrics,
    record_websocket_connection_closed, record_websocket_connection_error,
    record_websocket_connection_opened, record_websocket_connection_timeout,
};
pub use shutdown::{
    CloseGracePeriod, DEFAULT_CLOSE_GRACE_PERIOD, MAXIMUM_CLOSE_GRACE_PERIOD,
    MINIMUM_CLOSE_GRACE_PERIOD, WebSocketShutdownConfig, gracefully_close_websocket,
};
