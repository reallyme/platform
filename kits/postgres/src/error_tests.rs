// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    PostgresQueryErrorReason, PostgresRetryHint, classify_sqlstate, has_transport_failure,
};

#[test]
fn transport_errors_are_distinguished_from_non_network_io_errors() {
    let reset = std::io::Error::from(std::io::ErrorKind::ConnectionReset);
    let timed_out = std::io::Error::from(std::io::ErrorKind::TimedOut);
    let invalid_data = std::io::Error::from(std::io::ErrorKind::InvalidData);
    assert!(has_transport_failure(&reset));
    assert!(has_transport_failure(&timed_out));
    assert!(!has_transport_failure(&invalid_data));
}

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
