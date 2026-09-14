// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::extract::ws::CloseFrame;
use thiserror::Error;

/// Limit fields validated by the WebSocket foundation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketLimitField {
    /// The maximum inbound message size.
    MaxMessageSizeBytes,
    /// The maximum inbound frame size.
    MaxFrameSizeBytes,
    /// The bounded outbound queue capacity.
    OutboundQueueCapacity,
}

/// Heartbeat fields validated by the WebSocket foundation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketHeartbeatConfigField {
    /// The ping heartbeat interval.
    HeartbeatInterval,
    /// The idle timeout.
    IdleTimeout,
}

/// Shutdown fields validated by the WebSocket foundation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketShutdownConfigField {
    /// The close grace period used during graceful shutdown.
    CloseGracePeriod,
}

/// Why a WebSocket configuration value is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketValidationErrorReason {
    /// The value must be greater than zero.
    MustBeGreaterThanZero,
    /// The value must be at least the configured minimum.
    MustBeAtLeastMinimum,
    /// The value must be less than or equal to the configured maximum.
    MustBeLessThanOrEqualToMaximum,
    /// The maximum frame size must not exceed the maximum message size.
    MaxFrameSizeMustNotExceedMaxMessageSize,
    /// The idle timeout must exceed the heartbeat interval.
    IdleTimeoutMustExceedHeartbeatInterval,
}

/// Typed configuration validation failures for WebSocket infrastructure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketConfigError {
    /// The limit configuration was invalid.
    #[error("websocket limits are invalid")]
    InvalidLimits {
        /// The invalid field.
        field: WebSocketLimitField,
        /// Why the field was invalid.
        reason: WebSocketValidationErrorReason,
    },
    /// The heartbeat configuration was invalid.
    #[error("websocket heartbeat configuration is invalid")]
    InvalidHeartbeatConfig {
        /// The invalid field.
        field: WebSocketHeartbeatConfigField,
        /// Why the field was invalid.
        reason: WebSocketValidationErrorReason,
    },
    /// The shutdown configuration was invalid.
    #[error("websocket shutdown configuration is invalid")]
    InvalidShutdownConfig {
        /// The invalid field.
        field: WebSocketShutdownConfigField,
        /// Why the field was invalid.
        reason: WebSocketValidationErrorReason,
    },
}

/// Typed close reasons emitted by the WebSocket infrastructure layer.
///
/// These reasons are transport-safe and intentionally generic. They must never
/// be used to encode product/business message semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketCloseReason {
    /// Normal orderly closure initiated by the server.
    NormalClosure,
    /// The endpoint is going away.
    GoingAway,
    /// Graceful server-runtime shutdown is in progress.
    ServerShutdown,
    /// The connection has been idle for too long.
    IdleTimeout,
    /// The message exceeded the configured size limits.
    MessageTooLarge,
    /// The message violated the expected transport policy.
    InvalidMessage,
    /// The endpoint rejected the connection due to a generic policy issue.
    PolicyViolation,
    /// An internal transport failure occurred.
    InternalError,
}

impl WebSocketCloseReason {
    /// Returns the WebSocket close code associated with this reason.
    pub const fn code(self) -> u16 {
        match self {
            Self::NormalClosure => 1000,
            Self::GoingAway | Self::ServerShutdown | Self::IdleTimeout => 1001,
            Self::InvalidMessage | Self::PolicyViolation => 1008,
            Self::MessageTooLarge => 1009,
            Self::InternalError => 1011,
        }
    }

    /// Returns the stable public close reason text.
    pub const fn reason_text(self) -> &'static str {
        match self {
            Self::NormalClosure => "normal closure",
            Self::GoingAway => "going away",
            Self::ServerShutdown => "server shutdown",
            Self::IdleTimeout => "idle timeout",
            Self::MessageTooLarge => "message too large",
            Self::InvalidMessage => "invalid message",
            Self::PolicyViolation => "policy violation",
            Self::InternalError => "internal error",
        }
    }

    /// Returns a stable Axum close frame for this reason.
    pub fn to_close_frame(self) -> CloseFrame {
        CloseFrame {
            code: self.code(),
            reason: self.reason_text().into(),
        }
    }
}

/// Why an outbound send request could not be queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketSendErrorReason {
    /// The bounded outbound queue is full.
    OutboundQueueFull,
    /// The connection is no longer accepting outbound work.
    ConnectionClosed,
}

/// Why a WebSocket connection could not be accepted by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketConnectionLimitErrorReason {
    /// The configured active-connection capacity has been reached.
    TooManyActiveConnections,
}

/// Typed failure returned when WebSocket overload protection rejects a connection.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("websocket connection limit exceeded")]
pub struct WebSocketConnectionLimitError {
    reason: WebSocketConnectionLimitErrorReason,
}

impl WebSocketConnectionLimitError {
    /// Creates a typed WebSocket connection-limit failure.
    pub const fn new(reason: WebSocketConnectionLimitErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the reason the connection was rejected.
    pub const fn reason(self) -> WebSocketConnectionLimitErrorReason {
        self.reason
    }
}

/// Typed outbound-send failures for managed WebSocket connections.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("websocket outbound send failed")]
pub struct WebSocketSendError {
    reason: WebSocketSendErrorReason,
}

impl WebSocketSendError {
    /// Creates a typed outbound-send failure.
    pub const fn new(reason: WebSocketSendErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the reason for the outbound-send failure.
    pub const fn reason(self) -> WebSocketSendErrorReason {
        self.reason
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
