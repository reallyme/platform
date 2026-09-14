// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The public entry point for ReallyMe Platform.
//!
//! Platform separates host-neutral application behavior from the native or
//! edge host that executes it. The facade always exposes the application
//! contracts in [`app`]. The native runtime and infrastructure integrations are
//! available through explicit features so applications pay only for the
//! capabilities they select.
//!
//! Individual Platform crates remain independently usable. This facade keeps
//! their types identical by re-exporting the crates rather than wrapping them.

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

/// Host-neutral application contracts and conventions.
pub use reallyme_app_kit as app;

/// Production FoundationDB integration primitives.
#[cfg(feature = "foundationdb")]
pub use reallyme_foundationdb_kit as foundationdb;

/// Production NATS and JetStream integration primitives.
#[cfg(feature = "nats")]
pub use reallyme_nats_kit as nats;

/// Production PostgreSQL integration primitives.
#[cfg(feature = "postgres")]
pub use reallyme_postgres_kit as postgres;

/// Production S3-compatible object-storage integration primitives.
#[cfg(feature = "s3")]
pub use reallyme_s3_kit as s3;

/// Native server runtime and production lifecycle primitives.
#[cfg(feature = "native-server")]
pub use reallyme_server_kit as server;

/// Production Typesense integration primitives.
#[cfg(feature = "typesense")]
pub use reallyme_typesense_kit as typesense;

/// Production Valkey integration primitives.
#[cfg(feature = "valkey")]
pub use reallyme_valkey_kit as valkey;
