// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]

use reallyme_postgres_kit::{
    PostgresConfig, PostgresConfigInput, PostgresIsolationLevel, PostgresMigrationLockId,
    PostgresPool, PostgresQueryErrorReason, PostgresTransactionPolicy, PostgresTransportSecurity,
    begin_migration_transaction, begin_transaction, commit_transaction, rollback_transaction,
};
use secrecy::SecretString;

const INTEGRATION_ENABLED_ENV: &str = "REALLYME_RUN_POSTGRES_INTEGRATION";
const INTEGRATION_URI_ENV: &str = "REALLYME_POSTGRES_INTEGRATION_URI";
const INTEGRATION_MIGRATION_LOCK: PostgresMigrationLockId =
    PostgresMigrationLockId::new(0x524D_5047_5445_5354);

#[tokio::test]
async fn live_pool_transactions_and_error_classification() {
    if std::env::var(INTEGRATION_ENABLED_ENV).as_deref() != Ok("1") {
        eprintln!("SKIP: set REALLYME_RUN_POSTGRES_INTEGRATION=1 for live PostgreSQL tests");
        return;
    }

    let connection_uri = std::env::var(INTEGRATION_URI_ENV)
        .expect("live test requires REALLYME_POSTGRES_INTEGRATION_URI");
    let config = PostgresConfig::new(PostgresConfigInput {
        connection_uri: SecretString::from(connection_uri),
        application_name: Some("reallyme-postgres-kit-integration".to_owned()),
        max_pool_size: 4,
        min_pool_size: 1,
        connection_timeout_millis: 5_000,
        transport_security: PostgresTransportSecurity::AllowPlaintextForDevelopment,
        statement_timeout_millis: 2_500,
        lock_timeout_millis: 1_000,
        idle_transaction_timeout_millis: 2_500,
        ..PostgresConfigInput::default()
    })
    .expect("live test configuration should validate");

    let pool = PostgresPool::connect(&config)
        .await
        .expect("live PostgreSQL pool should connect eagerly");
    let health = pool
        .health_report()
        .await
        .expect("live PostgreSQL health query should succeed");
    assert!(health.connections >= 1);
    assert!(!health.tls_enabled);

    let mut connection = pool
        .get()
        .await
        .expect("live PostgreSQL connection should be available");
    let statement_timeout = connection
        .query_one("SELECT current_setting('statement_timeout')", &[])
        .await
        .expect("session timeout should be readable")
        .try_get::<_, String>(0)
        .expect("session timeout should be text");
    assert_eq!(statement_timeout, "2500ms");

    connection
        .batch_execute(
            "CREATE TEMP TABLE reallyme_postgres_kit_test (id BIGINT PRIMARY KEY) ON COMMIT PRESERVE ROWS",
        )
        .await
        .expect("temporary integration table should be created");

    let rollback = begin_transaction(
        &mut connection,
        PostgresTransactionPolicy::new(PostgresIsolationLevel::Serializable, false),
    )
    .await
    .expect("rollback transaction should begin");
    rollback
        .execute(
            "INSERT INTO reallyme_postgres_kit_test (id) VALUES ($1)",
            &[&1_i64],
        )
        .await
        .expect("rollback fixture should insert");
    rollback_transaction(rollback)
        .await
        .expect("explicit rollback should succeed");

    let count_after_rollback = row_count(&connection).await;
    assert_eq!(count_after_rollback, 0);

    let commit = begin_transaction(
        &mut connection,
        PostgresTransactionPolicy::new(PostgresIsolationLevel::Serializable, false),
    )
    .await
    .expect("commit transaction should begin");
    commit
        .execute(
            "INSERT INTO reallyme_postgres_kit_test (id) VALUES ($1)",
            &[&2_i64],
        )
        .await
        .expect("commit fixture should insert");
    commit_transaction(commit)
        .await
        .expect("explicit commit should succeed");

    let count_after_commit = row_count(&connection).await;
    assert_eq!(count_after_commit, 1);

    let duplicate = connection
        .execute(
            "INSERT INTO reallyme_postgres_kit_test (id) VALUES ($1)",
            &[&2_i64],
        )
        .await
        .expect_err("duplicate primary key should be rejected");
    assert_eq!(
        PostgresQueryErrorReason::classify(&duplicate),
        PostgresQueryErrorReason::ConstraintViolation
    );

    let migration = begin_migration_transaction(&mut connection, INTEGRATION_MIGRATION_LOCK)
        .await
        .expect("migration advisory lock should be acquired");
    rollback_transaction(migration)
        .await
        .expect("migration transaction should roll back cleanly");
}

async fn row_count(connection: &tokio_postgres::Client) -> i64 {
    connection
        .query_one("SELECT COUNT(*) FROM reallyme_postgres_kit_test", &[])
        .await
        .expect("temporary table count should succeed")
        .try_get::<_, i64>(0)
        .expect("count should decode as i64")
}
