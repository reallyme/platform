// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed PostgreSQL kit errors.

use thiserror::Error;

/// PostgreSQL kit result.
pub type PostgresResult<T> = Result<T, PostgresError>;

/// PostgreSQL kit error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PostgresError {
    /// Configuration failed validation.
    #[error("postgres configuration failed validation")]
    Config {
        /// Invalid configuration field.
        field: PostgresConfigField,
        /// Stable validation reason.
        reason: PostgresConfigErrorReason,
    },
    /// Pool or connection setup failed.
    #[error("postgres setup failed")]
    Setup {
        /// Stable setup reason.
        reason: PostgresSetupErrorReason,
    },
    /// Query execution failed.
    #[error("postgres query failed")]
    Query {
        /// Stable query reason.
        reason: PostgresQueryErrorReason,
    },
}

impl PostgresError {
    /// Converts a driver query error into the stable, non-sensitive kit error.
    pub fn from_query_error(error: &tokio_postgres::Error) -> Self {
        Self::Query {
            reason: PostgresQueryErrorReason::classify(error),
        }
    }
}

/// PostgreSQL configuration field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresConfigField {
    /// Prefix used to derive PostgreSQL environment-variable names.
    EnvironmentPrefix,
    /// PostgreSQL connection URI.
    ConnectionUri,
    /// Application name.
    ApplicationName,
    /// Maximum pool size.
    MaxPoolSize,
    /// Minimum idle pool size.
    MinPoolSize,
    /// Connection timeout.
    ConnectionTimeoutMillis,
    /// Transport security mode.
    TransportSecurity,
    /// Private TLS CA certificate path.
    TlsCaCertificatePath,
    /// Statement timeout.
    StatementTimeoutMillis,
    /// Lock timeout.
    LockTimeoutMillis,
    /// Idle transaction timeout.
    IdleTransactionTimeoutMillis,
}

/// PostgreSQL configuration validation reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresConfigErrorReason {
    /// Required value was empty.
    Empty,
    /// Numeric value was zero.
    Zero,
    /// Numeric value exceeded the accepted maximum.
    TooLarge,
    /// Value could not be parsed.
    InvalidSyntax,
    /// Value was not valid Unicode at the process environment boundary.
    InvalidEncoding,
    /// Value conflicts with another explicitly selected option.
    Incompatible,
}

/// PostgreSQL setup failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresSetupErrorReason {
    /// Connection URI could not be parsed by the PostgreSQL client.
    InvalidConnectionUri,
    /// Pool construction or first connection acquisition failed.
    PoolUnavailable,
    /// TLS trust roots could not be loaded.
    TlsTrustUnavailable,
    /// A custom TLS trust file was malformed or contained no certificates.
    TlsTrustInvalid,
    /// A custom TLS trust file exceeded the bounded startup policy.
    TlsTrustTooLarge,
    /// TLS client configuration could not be constructed.
    TlsConfigurationUnavailable,
}

/// PostgreSQL query failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresQueryErrorReason {
    /// A pooled connection could not be acquired.
    ConnectionUnavailable,
    /// A query failed.
    QueryFailed,
    /// PostgreSQL rejected the transaction due to a serialization conflict.
    SerializationConflict,
    /// PostgreSQL detected a transaction deadlock.
    DeadlockDetected,
    /// A schema constraint rejected the operation.
    ConstraintViolation,
    /// A requested lock could not be acquired within policy.
    LockUnavailable,
    /// PostgreSQL cancelled the query, including statement-timeout expiry.
    QueryCancelled,
    /// A transaction failed to begin, commit, or roll back.
    TransactionFailed,
    /// The application migration lock could not be acquired.
    MigrationLockUnavailable,
}

/// Bounded guidance for an app-owned retry policy.
///
/// A retry hint never makes an operation safe to retry by itself. Callers must
/// still prove idempotency and enforce bounded attempts, deadlines, and jitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresRetryHint {
    /// Do not automatically retry this failure.
    DoNotRetry,
    /// Restart the complete transaction from its first statement.
    RestartTransaction,
    /// Retry only when the complete operation is idempotent.
    RetryIdempotentOperation,
    /// Retry an idempotent operation after bounded randomized backoff.
    RetryIdempotentOperationAfterBackoff,
}

impl PostgresQueryErrorReason {
    /// Classifies a driver error without retaining server messages, SQL text,
    /// identifiers, or other potentially sensitive diagnostic fields.
    pub fn classify(error: &tokio_postgres::Error) -> Self {
        classify_sqlstate(
            error.code().map(tokio_postgres::error::SqlState::code),
            error.is_closed(),
        )
    }

    /// Returns conservative retry guidance for this stable error category.
    pub const fn retry_hint(self) -> PostgresRetryHint {
        match self {
            Self::SerializationConflict | Self::DeadlockDetected => {
                PostgresRetryHint::RestartTransaction
            }
            Self::ConnectionUnavailable => PostgresRetryHint::RetryIdempotentOperation,
            Self::LockUnavailable => PostgresRetryHint::RetryIdempotentOperationAfterBackoff,
            Self::QueryFailed
            | Self::ConstraintViolation
            | Self::QueryCancelled
            | Self::TransactionFailed
            | Self::MigrationLockUnavailable => PostgresRetryHint::DoNotRetry,
        }
    }
}

fn classify_sqlstate(code: Option<&str>, connection_closed: bool) -> PostgresQueryErrorReason {
    if connection_closed {
        return PostgresQueryErrorReason::ConnectionUnavailable;
    }

    match code {
        Some("40001") => PostgresQueryErrorReason::SerializationConflict,
        Some("40P01") => PostgresQueryErrorReason::DeadlockDetected,
        Some("55P03") => PostgresQueryErrorReason::LockUnavailable,
        Some("57014") => PostgresQueryErrorReason::QueryCancelled,
        Some("53300" | "57P01" | "57P02" | "57P03") => {
            PostgresQueryErrorReason::ConnectionUnavailable
        }
        Some(value) if value.starts_with("08") => PostgresQueryErrorReason::ConnectionUnavailable,
        Some(value) if value.starts_with("23") => PostgresQueryErrorReason::ConstraintViolation,
        Some(_) | None => PostgresQueryErrorReason::QueryFailed,
    }
}

#[cfg(test)]
mod tests {
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
}
