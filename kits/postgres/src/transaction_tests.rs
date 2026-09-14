// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
