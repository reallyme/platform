// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    MAX_IN_FLIGHT_REQUEST_LIMIT, MAX_IN_FLIGHT_REQUEST_LIMIT_VALUE, ResourceLimitEnforcement,
    RuntimeConcurrencyLimit,
};
use crate::config::{ConcurrencyLimitConfigField, ConfigError, ConfigValidationErrorReason};

#[test]
fn rejects_zero_concurrency_limit() {
    assert_eq!(
        RuntimeConcurrencyLimit::new(0, ConcurrencyLimitConfigField::HttpInFlightRequests),
        Err(ConfigError::InvalidConcurrencyLimitConfig {
            field: ConcurrencyLimitConfigField::HttpInFlightRequests,
            reason: ConfigValidationErrorReason::MustBeGreaterThanZero,
        })
    );
}

#[test]
fn rejects_concurrency_limit_above_platform_maximum() {
    assert_eq!(
        RuntimeConcurrencyLimit::new(
            MAX_IN_FLIGHT_REQUEST_LIMIT_VALUE + 1,
            ConcurrencyLimitConfigField::GrpcInFlightRequests,
        ),
        Err(ConfigError::InvalidConcurrencyLimitConfig {
            field: ConcurrencyLimitConfigField::GrpcInFlightRequests,
            reason: ConfigValidationErrorReason::MustBeLessThanOrEqualToMaximum,
        })
    );
}

#[test]
fn accepts_boundary_concurrency_limits() {
    assert!(
        RuntimeConcurrencyLimit::new(1, ConcurrencyLimitConfigField::HttpInFlightRequests).is_ok()
    );
    assert_eq!(
        RuntimeConcurrencyLimit::new(
            MAX_IN_FLIGHT_REQUEST_LIMIT.as_usize(),
            ConcurrencyLimitConfigField::GrpcInFlightRequests,
        )
        .map(|limit| limit.as_usize()),
        Ok(MAX_IN_FLIGHT_REQUEST_LIMIT.as_usize())
    );
}

#[test]
fn concurrency_limits_are_exact_fail_closed_admission_control() {
    let limit = RuntimeConcurrencyLimit::new(16, ConcurrencyLimitConfigField::HttpInFlightRequests)
        .expect("valid fixture limit");

    assert_eq!(
        limit.enforcement(),
        ResourceLimitEnforcement::ExactFailClosed
    );
}
