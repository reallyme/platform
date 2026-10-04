// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validated execution budget for one WebSocket application message.

use std::time::Duration;

use thiserror::Error;

use super::WebSocketConnectionRuntime;

const MAX_MESSAGE_HANDLER_TIMEOUT: Duration = Duration::from_secs(300);

/// Typed reason a WebSocket handler execution deadline is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketHandlerTimeoutErrorReason {
    /// A zero duration cannot bound handler execution.
    Zero,
    /// A long handler deadline defeats the connection lifetime bound.
    ExceedsMaximum,
}

/// Validation error for a WebSocket handler execution deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid websocket handler timeout: {reason:?}")]
pub struct WebSocketHandlerTimeoutError {
    reason: WebSocketHandlerTimeoutErrorReason,
}

impl WebSocketHandlerTimeoutError {
    /// Returns the finite validation reason.
    pub const fn reason(self) -> WebSocketHandlerTimeoutErrorReason {
        self.reason
    }
}

impl WebSocketConnectionRuntime {
    /// Sets the execution budget for one application message handler.
    /// Time spent draining the bounded outbound queue does not consume it.
    pub fn with_handler_timeout(
        mut self,
        timeout: Duration,
    ) -> Result<Self, WebSocketHandlerTimeoutError> {
        if timeout.is_zero() {
            return Err(WebSocketHandlerTimeoutError {
                reason: WebSocketHandlerTimeoutErrorReason::Zero,
            });
        }
        if timeout > MAX_MESSAGE_HANDLER_TIMEOUT {
            return Err(WebSocketHandlerTimeoutError {
                reason: WebSocketHandlerTimeoutErrorReason::ExceedsMaximum,
            });
        }
        self.handler_timeout = timeout;
        Ok(self)
    }

    /// Returns the per-message handler execution budget.
    pub const fn handler_timeout(&self) -> Duration {
        self.handler_timeout
    }
}
