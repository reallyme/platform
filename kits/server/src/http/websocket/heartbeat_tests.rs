// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::{
    DEFAULT_HEARTBEAT_INTERVAL, DEFAULT_IDLE_TIMEOUT, HeartbeatInterval, IdleTimeout,
    MAXIMUM_HEARTBEAT_INTERVAL, MAXIMUM_IDLE_TIMEOUT, MINIMUM_HEARTBEAT_INTERVAL,
    MINIMUM_IDLE_TIMEOUT, WebSocketHeartbeatConfig,
};
use crate::http::websocket::{
    WebSocketConfigError, WebSocketHeartbeatConfigField, WebSocketValidationErrorReason,
};

#[test]
fn rejects_zero_heartbeat_values() {
    assert_eq!(
        HeartbeatInterval::new(Duration::ZERO),
        Err(WebSocketConfigError::InvalidHeartbeatConfig {
            field: WebSocketHeartbeatConfigField::HeartbeatInterval,
            reason: WebSocketValidationErrorReason::MustBeGreaterThanZero,
        })
    );
}

#[test]
fn rejects_timing_values_below_minimum() {
    assert_eq!(
        HeartbeatInterval::new(MINIMUM_HEARTBEAT_INTERVAL / 2),
        Err(WebSocketConfigError::InvalidHeartbeatConfig {
            field: WebSocketHeartbeatConfigField::HeartbeatInterval,
            reason: WebSocketValidationErrorReason::MustBeAtLeastMinimum,
        })
    );
    assert_eq!(
        IdleTimeout::new(MINIMUM_IDLE_TIMEOUT / 2),
        Err(WebSocketConfigError::InvalidHeartbeatConfig {
            field: WebSocketHeartbeatConfigField::IdleTimeout,
            reason: WebSocketValidationErrorReason::MustBeAtLeastMinimum,
        })
    );
}

#[test]
fn rejects_timing_values_above_maximum() {
    assert_eq!(
        HeartbeatInterval::new(MAXIMUM_HEARTBEAT_INTERVAL + Duration::from_secs(1)),
        Err(WebSocketConfigError::InvalidHeartbeatConfig {
            field: WebSocketHeartbeatConfigField::HeartbeatInterval,
            reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
        })
    );
    assert_eq!(
        IdleTimeout::new(MAXIMUM_IDLE_TIMEOUT + Duration::from_secs(1)),
        Err(WebSocketConfigError::InvalidHeartbeatConfig {
            field: WebSocketHeartbeatConfigField::IdleTimeout,
            reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
        })
    );
}

#[test]
fn accepts_timing_boundary_values() {
    assert!(HeartbeatInterval::new(MINIMUM_HEARTBEAT_INTERVAL).is_ok());
    assert!(HeartbeatInterval::new(MAXIMUM_HEARTBEAT_INTERVAL).is_ok());
    assert!(IdleTimeout::new(MINIMUM_IDLE_TIMEOUT).is_ok());
    assert!(IdleTimeout::new(MAXIMUM_IDLE_TIMEOUT).is_ok());
}

#[test]
fn rejects_idle_timeout_that_does_not_exceed_heartbeat_interval() {
    let result = WebSocketHeartbeatConfig::new(
        HeartbeatInterval::new(Duration::from_secs(30)).expect("fixture should be valid"),
        IdleTimeout::new(Duration::from_secs(30)).expect("fixture should be valid"),
    );

    assert_eq!(
        result,
        Err(WebSocketConfigError::InvalidHeartbeatConfig {
            field: WebSocketHeartbeatConfigField::IdleTimeout,
            reason: WebSocketValidationErrorReason::IdleTimeoutMustExceedHeartbeatInterval,
        })
    );
}

#[test]
fn safe_defaults_are_valid() {
    let config = WebSocketHeartbeatConfig::safe_defaults();

    assert_eq!(
        config.heartbeat_interval().as_duration(),
        DEFAULT_HEARTBEAT_INTERVAL
    );
    assert_eq!(config.idle_timeout().as_duration(), DEFAULT_IDLE_TIMEOUT);
}
