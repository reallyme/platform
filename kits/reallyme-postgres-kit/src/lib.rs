// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! PostgreSQL primitives for ReallyMe services.
//!
//! This kit owns reusable PostgreSQL configuration, typed errors, pooled
//! connection setup, and startup health checks. App crates should depend on
//! these small primitives instead of each app inventing its own connection
//! parsing, secret handling, pool sizing, or readiness behavior.

mod config;
mod connector;
mod error;
mod transaction;

pub use config::{
    PostgresConfig, PostgresConfigInput, PostgresTlsTrust, PostgresTransportSecurity,
};
pub use connector::{PostgresHealthReport, PostgresPool, PostgresPooledConnection};
pub use error::{
    PostgresConfigErrorReason, PostgresConfigField, PostgresError, PostgresQueryErrorReason,
    PostgresResult, PostgresRetryHint, PostgresSetupErrorReason,
};
pub use transaction::{
    PostgresIsolationLevel, PostgresMigrationLockId, PostgresTransactionPolicy,
    begin_migration_transaction, begin_transaction, commit_transaction, rollback_transaction,
};
