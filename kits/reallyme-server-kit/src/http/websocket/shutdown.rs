// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use tokio::time::{Instant, timeout_at};

use super::error::{
    WebSocketCloseReason, WebSocketConfigError, WebSocketShutdownConfigField,
    WebSocketValidationErrorReason,
};

/// Conservative default grace period for close handshakes.
pub const DEFAULT_CLOSE_GRACE_PERIOD: Duration = Duration::from_secs(5);
/// Minimum close grace period allowed by the platform.
///
/// Extremely short close periods make graceful shutdown behave like an abort
/// under normal scheduler jitter.
pub const MINIMUM_CLOSE_GRACE_PERIOD: Duration = Duration::from_secs(1);
/// Maximum close grace period allowed by the platform.
///
/// This bounds how long shutdown can wait on a non-cooperative peer before the
/// socket is dropped.
pub const MAXIMUM_CLOSE_GRACE_PERIOD: Duration = Duration::from_secs(30);

/// Validated grace period for a WebSocket close handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloseGracePeriod(Duration);

impl CloseGracePeriod {
    /// Creates a validated close grace period.
    pub fn new(value: Duration) -> Result<Self, WebSocketConfigError> {
        if value.is_zero() {
            return Err(WebSocketConfigError::InvalidShutdownConfig {
                field: WebSocketShutdownConfigField::CloseGracePeriod,
                reason: WebSocketValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        if value < MINIMUM_CLOSE_GRACE_PERIOD {
            return Err(WebSocketConfigError::InvalidShutdownConfig {
                field: WebSocketShutdownConfigField::CloseGracePeriod,
                reason: WebSocketValidationErrorReason::MustBeAtLeastMinimum,
            });
        }

        if value > MAXIMUM_CLOSE_GRACE_PERIOD {
            return Err(WebSocketConfigError::InvalidShutdownConfig {
                field: WebSocketShutdownConfigField::CloseGracePeriod,
                reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            });
        }

        Ok(Self(value))
    }

    /// Returns the grace period as a duration.
    pub const fn as_duration(self) -> Duration {
        self.0
    }

    fn safe_default() -> Self {
        Self(DEFAULT_CLOSE_GRACE_PERIOD)
    }
}

/// Validated shutdown behavior for WebSocket connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebSocketShutdownConfig {
    close_grace_period: CloseGracePeriod,
}

impl WebSocketShutdownConfig {
    /// Constructs validated WebSocket shutdown behavior.
    pub const fn new(close_grace_period: CloseGracePeriod) -> Self {
        Self { close_grace_period }
    }

    /// Returns a conservative safe default shutdown configuration.
    pub fn safe_defaults() -> Self {
        Self {
            close_grace_period: CloseGracePeriod::safe_default(),
        }
    }

    /// Returns the close grace period.
    pub const fn close_grace_period(self) -> CloseGracePeriod {
        self.close_grace_period
    }
}

impl Default for WebSocketShutdownConfig {
    fn default() -> Self {
        Self::safe_defaults()
    }
}

/// Sends a close frame and waits briefly for the peer to finish the close
/// handshake.
///
/// If the peer does not finish the handshake before the grace period expires,
/// the socket is dropped so shutdown does not stall indefinitely.
pub async fn gracefully_close_websocket(
    socket: &mut WebSocket,
    reason: WebSocketCloseReason,
    shutdown: WebSocketShutdownConfig,
) {
    let _ = socket
        .send(Message::Close(Some(reason.to_close_frame())))
        .await;

    let deadline = Instant::now() + shutdown.close_grace_period().as_duration();
    while let Ok(receive_result) = timeout_at(deadline, socket.recv()).await {
        match receive_result {
            Some(Ok(Message::Close(_))) | None => break,
            Some(Ok(_)) => continue,
            Some(Err(_)) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        CloseGracePeriod, DEFAULT_CLOSE_GRACE_PERIOD, MAXIMUM_CLOSE_GRACE_PERIOD,
        MINIMUM_CLOSE_GRACE_PERIOD, WebSocketShutdownConfig,
    };
    use crate::http::websocket::{
        WebSocketConfigError, WebSocketShutdownConfigField, WebSocketValidationErrorReason,
    };

    #[test]
    fn rejects_zero_close_grace_period() {
        assert_eq!(
            CloseGracePeriod::new(Duration::ZERO),
            Err(WebSocketConfigError::InvalidShutdownConfig {
                field: WebSocketShutdownConfigField::CloseGracePeriod,
                reason: WebSocketValidationErrorReason::MustBeGreaterThanZero,
            })
        );
    }

    #[test]
    fn rejects_close_grace_period_below_minimum() {
        assert_eq!(
            CloseGracePeriod::new(MINIMUM_CLOSE_GRACE_PERIOD / 2),
            Err(WebSocketConfigError::InvalidShutdownConfig {
                field: WebSocketShutdownConfigField::CloseGracePeriod,
                reason: WebSocketValidationErrorReason::MustBeAtLeastMinimum,
            })
        );
    }

    #[test]
    fn rejects_close_grace_period_above_maximum() {
        assert_eq!(
            CloseGracePeriod::new(MAXIMUM_CLOSE_GRACE_PERIOD + Duration::from_secs(1)),
            Err(WebSocketConfigError::InvalidShutdownConfig {
                field: WebSocketShutdownConfigField::CloseGracePeriod,
                reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            })
        );
    }

    #[test]
    fn accepts_close_grace_period_boundary_values() {
        assert!(CloseGracePeriod::new(MINIMUM_CLOSE_GRACE_PERIOD).is_ok());
        assert!(CloseGracePeriod::new(MAXIMUM_CLOSE_GRACE_PERIOD).is_ok());
    }

    #[test]
    fn safe_defaults_are_valid() {
        let shutdown = WebSocketShutdownConfig::safe_defaults();

        assert_eq!(
            shutdown.close_grace_period().as_duration(),
            DEFAULT_CLOSE_GRACE_PERIOD
        );
    }
}
