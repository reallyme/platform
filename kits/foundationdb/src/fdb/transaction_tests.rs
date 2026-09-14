// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ReadTxnPolicy, TransactionPolicyError, WriteTxnPolicy};
use std::time::Duration;

#[test]
fn read_default_policy_is_fast_and_idempotent() {
    let policy = ReadTxnPolicy::default();
    let options = policy.to_transact_option();
    assert!(options.is_idempotent);
    assert_eq!(policy.retry_limit(), 2);
    assert_eq!(policy.time_out(), Duration::from_millis(500));
    assert_eq!(options.retry_limit, Some(2));
    assert_eq!(options.time_out, Some(Duration::from_millis(500)));
}

#[test]
fn write_default_policy_is_retryable_and_non_idempotent() {
    let policy = WriteTxnPolicy::default();
    let options = policy.to_transact_option();
    assert!(!options.is_idempotent);
    assert_eq!(policy.retry_limit(), 3);
    assert_eq!(policy.time_out(), Duration::from_secs(5));
    assert_eq!(options.retry_limit, Some(3));
    assert_eq!(options.time_out, Some(Duration::from_secs(5)));
}

#[test]
fn policy_validation_rejects_zero_retries() {
    assert!(matches!(
        ReadTxnPolicy::try_new(0, Duration::from_millis(10)),
        Err(TransactionPolicyError::InvalidRetryLimit { .. })
    ));
    assert!(matches!(
        WriteTxnPolicy::try_new(0, Duration::from_millis(10)),
        Err(TransactionPolicyError::InvalidRetryLimit { .. })
    ));
}

#[test]
fn policy_validation_rejects_too_long_timeout() {
    assert!(matches!(
        ReadTxnPolicy::try_new(1, Duration::from_millis(100_000)),
        Err(TransactionPolicyError::InvalidTimeout { .. })
    ));
}
