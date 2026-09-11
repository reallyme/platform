// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![deny(missing_docs)]
#![deny(unsafe_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! FoundationDB primitives for ReallyMe services.
//!
//! This kit owns the reusable FoundationDB connection, configuration, typed
//! errors, transaction policy, retry policy, tuple/key encoding boundary,
//! key namespace/prefix validation, range scan helpers, key metadata, and
//! tenant-scoped access.
//!
//! Tenants are the segmentation primitive that higher-level schemas build on.
//! The kit validates neutral tenant names, opens tenant-scoped FoundationDB
//! handles, and fails closed if a tenant has not been provisioned. Applications
//! own the mapping from their domain concepts to these infrastructure names.
//!
//! No async-graphql, axum, or HTTP client code belongs here.

/// FoundationDB connector, transactions, tenants, and low-level primitives.
pub mod fdb;
/// Validated key construction helpers shared by FoundationDB adapters.
pub mod keys;

pub use fdb::config::{FdbConfig, FdbConfigInput};
pub use fdb::connector::{FdbContext, FdbHealthReport, FoundationDbConnector};
pub use fdb::error::{
    ConfigErrorReason, ConfigField, FdbError, FdbHealthErrorReason, FdbQueryErrorReason, FdbResult,
    FdbSetupErrorReason, TenantErrorReason, TenantMetadataField,
};
pub use fdb::startup::{verify_ready, verify_tenants_provisioned};
pub use fdb::tenant::TenantHandle;
pub use fdb::tenant_name::{
    FoundationDbTenantName, FoundationDbTenantNameError, FoundationDbTenantNameErrorReason,
    MAX_FOUNDATIONDB_TENANT_NAME_LENGTH,
};
