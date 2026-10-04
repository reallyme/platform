// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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

#[test]
fn rejects_shutdown_timeout_that_could_overflow_deadline() {
    assert_eq!(
        ShutdownTimeout::new(Duration::from_secs(u64::MAX)),
        Err(ShutdownError::InvalidTimeout {
            reason: ShutdownValidationErrorReason::TooLarge,
        })
    );
}
