// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use tokio::sync::watch;

use crate::shutdown::{ShutdownError, ShutdownReason, ShutdownValidationErrorReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShutdownState {
    Running,
    Requested(ShutdownReason),
}

/// Broadcast source for coordinated server-runtime shutdown.
///
/// Services create a single controller and clone [`ShutdownToken`] values into
/// long-lived tasks. This keeps shutdown fan-out explicit and avoids hidden
/// process-global state.
#[derive(Debug)]
pub struct ShutdownController {
    sender: watch::Sender<ShutdownState>,
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownController {
    /// Creates a new controller in the running state.
    pub fn new() -> Self {
        let (sender, _) = watch::channel(ShutdownState::Running);

        Self { sender }
    }

    /// Returns a new token subscribed to this controller.
    pub fn token(&self) -> ShutdownToken {
        ShutdownToken {
            receiver: self.sender.subscribe(),
        }
    }

    /// Returns whether shutdown has already been requested.
    pub fn is_shutdown_requested(&self) -> bool {
        matches!(*self.sender.borrow(), ShutdownState::Requested(_))
    }

    /// Returns the active shutdown reason, if shutdown has begun.
    pub fn shutdown_reason(&self) -> Option<ShutdownReason> {
        match *self.sender.borrow() {
            ShutdownState::Running => None,
            ShutdownState::Requested(reason) => Some(reason),
        }
    }

    /// Requests coordinated shutdown.
    ///
    /// Returns `true` only for the first successful transition from running to
    /// shutting down. Later calls are ignored so all tasks observe one stable
    /// shutdown reason.
    pub fn begin_shutdown(&self, reason: ShutdownReason) -> bool {
        self.sender.send_if_modified(|state| match *state {
            ShutdownState::Running => {
                *state = ShutdownState::Requested(reason);
                true
            }
            ShutdownState::Requested(_) => false,
        })
    }
}

/// Cloneable cancellation token used by background tasks.
///
/// Tasks should either await [`Self::cancelled`] directly or poll
/// [`Self::is_shutdown_requested`] inside cancellation-safe loops.
#[derive(Debug, Clone)]
pub struct ShutdownToken {
    receiver: watch::Receiver<ShutdownState>,
}

impl ShutdownToken {
    /// Returns whether shutdown has already been requested.
    pub fn is_shutdown_requested(&self) -> bool {
        matches!(*self.receiver.borrow(), ShutdownState::Requested(_))
    }

    /// Returns the active shutdown reason, if available.
    pub fn shutdown_reason(&self) -> Option<ShutdownReason> {
        match *self.receiver.borrow() {
            ShutdownState::Running => None,
            ShutdownState::Requested(reason) => Some(reason),
        }
    }

    /// Waits until shutdown is requested and returns the typed reason.
    ///
    /// If the controller disappears unexpectedly, the token returns
    /// [`ShutdownReason::Unknown`] instead of hanging indefinitely.
    pub async fn cancelled(&mut self) -> ShutdownReason {
        if let Some(reason) = self.shutdown_reason() {
            return reason;
        }

        loop {
            if self.receiver.changed().await.is_err() {
                return self.shutdown_reason().unwrap_or(ShutdownReason::Unknown);
            }

            if let Some(reason) = self.shutdown_reason() {
                return reason;
            }
        }
    }
}

/// Validated graceful-shutdown deadline for draining background tasks.
///
/// The timeout must be explicit and non-zero so callers cannot accidentally
/// request an immediate abort path when they intended a graceful drain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownTimeout(Duration);

impl ShutdownTimeout {
    /// Constructs a validated shutdown timeout.
    pub fn new(value: Duration) -> Result<Self, ShutdownError> {
        if value.is_zero() {
            return Err(ShutdownError::InvalidTimeout {
                reason: ShutdownValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        Ok(Self(value))
    }

    /// Returns the configured timeout as a standard duration.
    pub fn as_duration(&self) -> Duration {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{ShutdownController, ShutdownTimeout};
    use crate::shutdown::{ShutdownError, ShutdownReason, ShutdownValidationErrorReason};

    #[tokio::test]
    async fn token_observes_shutdown_reason() {
        let controller = ShutdownController::new();
        let mut token = controller.token();

        assert!(controller.begin_shutdown(ShutdownReason::Sigterm));

        let reason = token.cancelled().await;

        assert_eq!(reason, ShutdownReason::Sigterm);
    }

    #[tokio::test]
    async fn repeated_shutdown_requests_preserve_first_reason() {
        let controller = ShutdownController::new();
        let mut token = controller.token();

        assert!(controller.begin_shutdown(ShutdownReason::CtrlC));
        assert!(!controller.begin_shutdown(ShutdownReason::Sigterm));

        let reason = token.cancelled().await;

        assert_eq!(reason, ShutdownReason::CtrlC);
        assert_eq!(controller.shutdown_reason(), Some(ShutdownReason::CtrlC));
    }

    #[test]
    fn rejects_zero_shutdown_timeout() {
        let result = ShutdownTimeout::new(Duration::ZERO);

        assert_eq!(
            result,
            Err(ShutdownError::InvalidTimeout {
                reason: ShutdownValidationErrorReason::MustBeGreaterThanZero,
            })
        );
    }
}
