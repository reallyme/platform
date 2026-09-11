// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Explicit PostgreSQL transaction and migration policies.

use tokio_postgres::{IsolationLevel, Transaction};

use crate::connector::PostgresPooledConnection;
use crate::error::{PostgresError, PostgresQueryErrorReason, PostgresResult};

/// PostgreSQL transaction isolation selected by application semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresIsolationLevel {
    /// PostgreSQL's default isolation, suitable for independent writes.
    ReadCommitted,
    /// A stable transaction snapshot, with serialization still enforced by the app.
    RepeatableRead,
    /// PostgreSQL serialization with explicit retry required on conflicts.
    Serializable,
}

/// Explicit policy used when beginning a PostgreSQL transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostgresTransactionPolicy {
    isolation_level: PostgresIsolationLevel,
    read_only: bool,
}

impl PostgresTransactionPolicy {
    /// Creates an explicit transaction policy.
    pub const fn new(isolation_level: PostgresIsolationLevel, read_only: bool) -> Self {
        Self {
            isolation_level,
            read_only,
        }
    }

    /// Returns the selected isolation level.
    pub const fn isolation_level(self) -> PostgresIsolationLevel {
        self.isolation_level
    }

    /// Returns whether PostgreSQL must reject writes in the transaction.
    pub const fn read_only(self) -> bool {
        self.read_only
    }
}

/// Stable application-owned advisory lock identifier for schema migrations.
///
/// Each application must use a distinct, version-stable value. PostgreSQL
/// holds this lock only for the surrounding transaction, preventing two app
/// instances from racing the same idempotent schema migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostgresMigrationLockId(i64);

impl PostgresMigrationLockId {
    /// Creates a migration lock identifier.
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    /// Returns the PostgreSQL advisory lock value.
    pub const fn value(self) -> i64 {
        self.0
    }
}

/// Begins a transaction using an explicit isolation and mutability policy.
pub async fn begin_transaction<'connection, 'pool>(
    connection: &'connection mut PostgresPooledConnection<'pool>,
    policy: PostgresTransactionPolicy,
) -> PostgresResult<Transaction<'connection>> {
    connection
        .build_transaction()
        .isolation_level(isolation_level(policy.isolation_level()))
        .read_only(policy.read_only())
        .start()
        .await
        .map_err(|error| PostgresError::from_query_error(&error))
}

/// Begins a serializable migration transaction and obtains its app lock.
pub async fn begin_migration_transaction<'connection, 'pool>(
    connection: &'connection mut PostgresPooledConnection<'pool>,
    lock_id: PostgresMigrationLockId,
) -> PostgresResult<Transaction<'connection>> {
    let transaction = begin_transaction(
        connection,
        PostgresTransactionPolicy::new(PostgresIsolationLevel::Serializable, false),
    )
    .await?;
    transaction
        .query_one("SELECT pg_advisory_xact_lock($1)", &[&lock_id.value()])
        .await
        .map_err(|_error| PostgresError::Query {
            reason: PostgresQueryErrorReason::MigrationLockUnavailable,
        })?;
    Ok(transaction)
}

/// Commits a transaction and classifies any PostgreSQL failure without
/// retaining server-provided diagnostic text.
pub async fn commit_transaction(transaction: Transaction<'_>) -> PostgresResult<()> {
    transaction
        .commit()
        .await
        .map_err(|error| PostgresError::from_query_error(&error))
}

/// Explicitly rolls back a transaction and classifies any PostgreSQL failure.
///
/// Dropping a transaction also rolls it back, but this helper is preferable
/// when the caller must know whether the connection accepted the rollback.
pub async fn rollback_transaction(transaction: Transaction<'_>) -> PostgresResult<()> {
    transaction
        .rollback()
        .await
        .map_err(|error| PostgresError::from_query_error(&error))
}

fn isolation_level(value: PostgresIsolationLevel) -> IsolationLevel {
    match value {
        PostgresIsolationLevel::ReadCommitted => IsolationLevel::ReadCommitted,
        PostgresIsolationLevel::RepeatableRead => IsolationLevel::RepeatableRead,
        PostgresIsolationLevel::Serializable => IsolationLevel::Serializable,
    }
}

#[cfg(test)]
mod tests {
    use super::{PostgresIsolationLevel, PostgresMigrationLockId, PostgresTransactionPolicy};

    #[test]
    fn transaction_policy_preserves_explicit_semantics() {
        let policy = PostgresTransactionPolicy::new(PostgresIsolationLevel::RepeatableRead, true);

        assert_eq!(
            policy.isolation_level(),
            PostgresIsolationLevel::RepeatableRead
        );
        assert!(policy.read_only());
    }

    #[test]
    fn migration_lock_id_is_stable() {
        let lock_id = PostgresMigrationLockId::new(42);

        assert_eq!(lock_id.value(), 42);
    }
}
