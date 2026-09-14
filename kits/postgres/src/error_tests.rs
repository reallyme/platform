// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{PostgresQueryErrorReason, PostgresRetryHint, classify_sqlstate};

#[test]
fn sqlstate_classification_is_stable_and_low_cardinality() {
    assert_eq!(
        classify_sqlstate(Some("40001"), false),
        PostgresQueryErrorReason::SerializationConflict
    );
    assert_eq!(
        classify_sqlstate(Some("40P01"), false),
        PostgresQueryErrorReason::DeadlockDetected
    );
    assert_eq!(
        classify_sqlstate(Some("23505"), false),
        PostgresQueryErrorReason::ConstraintViolation
    );
    assert_eq!(
        classify_sqlstate(Some("08006"), false),
        PostgresQueryErrorReason::ConnectionUnavailable
    );
    assert_eq!(
        classify_sqlstate(Some("53100"), false),
        PostgresQueryErrorReason::QueryFailed
    );
    assert_eq!(
        classify_sqlstate(Some("XX000"), false),
        PostgresQueryErrorReason::QueryFailed
    );
}

#[test]
fn closed_connection_takes_precedence_over_sqlstate() {
    assert_eq!(
        classify_sqlstate(Some("23505"), true),
        PostgresQueryErrorReason::ConnectionUnavailable
    );
}

#[test]
fn retry_hints_require_idempotency_or_transaction_restart() {
    assert_eq!(
        PostgresQueryErrorReason::SerializationConflict.retry_hint(),
        PostgresRetryHint::RestartTransaction
    );
    assert_eq!(
        PostgresQueryErrorReason::ConnectionUnavailable.retry_hint(),
        PostgresRetryHint::RetryIdempotentOperation
    );
    assert_eq!(
        PostgresQueryErrorReason::ConstraintViolation.retry_hint(),
        PostgresRetryHint::DoNotRetry
    );
}
