// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
