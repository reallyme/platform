// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::transact::{c_api_retry_limit, c_api_timeout_millis, retry_permitted};

#[test]
fn retry_requires_retryable_error_and_remaining_budget() {
    assert!(retry_permitted(true, false, false, true));
    assert!(!retry_permitted(false, false, true, true));
    assert!(!retry_permitted(true, false, true, false));
}

#[test]
fn maybe_committed_is_retried_only_for_idempotent_operations() {
    assert!(retry_permitted(true, true, true, true));
    assert!(!retry_permitted(true, true, false, true));
}

#[test]
fn c_api_limits_fail_closed_for_unvalidated_options() {
    assert_eq!(c_api_timeout_millis(Duration::ZERO), 1);
    assert_eq!(c_api_timeout_millis(Duration::from_millis(500)), 500);
    assert_eq!(c_api_timeout_millis(Duration::MAX), i32::MAX);
    assert_eq!(c_api_retry_limit(1), 0);
    assert_eq!(c_api_retry_limit(u32::MAX), i32::MAX);
}
