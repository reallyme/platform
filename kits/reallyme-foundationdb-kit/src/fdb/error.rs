// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed FoundationDB infrastructure errors.

use std::num::NonZeroUsize;

use thiserror::Error;

use crate::fdb::tenant_name::FoundationDbTenantName;

/// Top-level FoundationDB infrastructure error.
///
/// Higher-level kits and apps should wrap or compose this error rather than
/// returning it directly from public business surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FdbError {
    /// Validated connector configuration could not be constructed.
    #[error("foundationdb configuration error: {reason}")]
    Config {
        /// Stable configuration failure reason.
        reason: ConfigErrorReason,
    },
    /// FoundationDB client or database setup failed.
    #[error("foundationdb setup error: {reason}")]
    Setup {
        /// Stable setup failure reason.
        reason: FdbSetupErrorReason,
    },
    /// A low-level query primitive failed.
    #[error("foundationdb query error: {reason}")]
    Query {
        /// Stable query failure reason.
        reason: FdbQueryErrorReason,
    },
    /// Connector health verification failed.
    #[error("foundationdb health check failed: {reason}")]
    Health {
        /// Stable health failure reason.
        reason: FdbHealthErrorReason,
    },
    /// Tenant access, metadata, or administration failed.
    #[error("foundationdb tenant error: {reason}")]
    Tenant {
        /// Stable tenant failure reason.
        reason: TenantErrorReason,
    },
}

/// Tenant-related failures.
///
/// `TenantNotProvisioned` is the production fail-closed signal: the kit will
/// not create tenants implicitly. Operators must provision tenants via the
/// admin helpers (under the `tenant-admin` feature) or `fdbcli`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TenantErrorReason {
    /// The requested tenant has not been explicitly provisioned.
    #[error("tenant `{tenant}` is not provisioned in foundationdb")]
    NotProvisioned {
        /// Requested logical tenant.
        tenant: FoundationDbTenantName,
    },
    /// Tenant metadata could not be queried.
    #[error("tenant lookup failed for `{tenant}`")]
    LookupFailed {
        /// Requested logical tenant.
        tenant: FoundationDbTenantName,
    },
    /// FoundationDB could not create a tenant-scoped handle.
    #[error("tenant `{tenant}` could not be opened")]
    OpenFailed {
        /// Requested logical tenant.
        tenant: FoundationDbTenantName,
    },
    /// The requested create operation targeted an existing tenant.
    #[error("tenant `{tenant}` already exists")]
    AlreadyExists {
        /// Requested logical tenant.
        tenant: FoundationDbTenantName,
    },
    /// An operator-only tenant lifecycle operation failed.
    #[error("tenant administration failed for `{tenant}`")]
    AdministrationFailed {
        /// Requested logical tenant.
        tenant: FoundationDbTenantName,
    },
    /// Tenant metadata is missing required key(s).
    #[error("tenant `{tenant}` metadata is missing required field `{field:?}")]
    MetadataMissing {
        /// Tenant whose metadata could not be read.
        tenant: FoundationDbTenantName,
        /// Missing metadata field.
        field: TenantMetadataField,
    },
    /// Tenant metadata is malformed and cannot be decoded.
    #[error("tenant `{tenant}` metadata field `{field:?}` is malformed")]
    MetadataMalformed {
        /// Tenant whose metadata is malformed.
        tenant: FoundationDbTenantName,
        /// Malformed metadata field.
        field: TenantMetadataField,
    },
    /// Tenant metadata schema version is not supported by this binary.
    #[error("tenant `{tenant}` metadata schema version `{schema_version}` is unsupported")]
    SchemaVersionUnsupported {
        /// Tenant with incompatible schema version.
        tenant: FoundationDbTenantName,
        /// Encoded schema version value.
        schema_version: u32,
    },
}

/// Stable tenant metadata fields used in typed failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantMetadataField {
    /// Tenant schema version.
    SchemaVersion,
    /// Tenant creation timestamp.
    CreatedAt,
}

/// Validated configuration fields owned by this kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    /// Prefix used to derive environment-variable names.
    EnvironmentPrefix,
    /// FoundationDB runtime API version.
    ApiVersion,
    /// FoundationDB cluster file path.
    ClusterFile,
    /// Connector readiness timeout.
    HealthCheckTimeoutMillis,
}

/// Stable configuration validation failure reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ConfigErrorReason {
    /// A numeric environment value could not be parsed.
    #[error("invalid integer value in {field:?}")]
    InvalidInteger {
        /// Invalid configuration field.
        field: ConfigField,
    },
    /// A numeric value fell outside its safe operating range.
    #[error("value out of bounds in {field:?}")]
    ValueOutOfBounds {
        /// Invalid configuration field.
        field: ConfigField,
    },
    /// A cluster-file path was empty, oversized, or contained a null byte.
    #[error("fdb cluster file path is not valid")]
    InvalidClusterFilePath,
    /// A configured cluster file was absent, unreadable, or not a file.
    #[error("fdb cluster file path is missing or cannot be read")]
    MissingClusterFile,
    /// A required configuration value was empty.
    #[error("configuration value is empty")]
    Empty,
    /// An environment value was not valid Unicode.
    #[error("configuration value in {field:?} is not valid unicode")]
    InvalidEncoding {
        /// Invalid configuration field.
        field: ConfigField,
    },
    /// An environment prefix was not uppercase ASCII with digits/underscores.
    #[error("environment prefix is invalid")]
    InvalidEnvironmentPrefix,
}

/// Stable FoundationDB client and database setup failure reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FdbSetupErrorReason {
    /// The process already initialized the singleton FoundationDB client.
    #[error("foundationdb client is already initialized in this process")]
    ClientAlreadyInitialized,
    /// The selected runtime API is incompatible with the linked client.
    #[error("api version not supported")]
    ApiVersionUnsupported,
    /// The FoundationDB network thread could not be booted.
    #[error("network bootstrap failed")]
    NetworkBootFailed,
    /// A database handle could not be opened.
    #[error("database open failed")]
    DatabaseOpenFailed,
    /// The upstream binding panicked while initializing its global client.
    #[error("foundationdb client initialization panicked")]
    ClientInitializationPanicked,
}

/// Stable, low-cardinality connector health failure reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FdbHealthErrorReason {
    /// The client network thread did not process a local no-op.
    #[error("client network thread is unavailable")]
    NetworkUnavailable,
    /// A read version could not be obtained from the cluster.
    #[error("cluster is unavailable")]
    ClusterUnavailable,
    /// The complete health probe exceeded its configured deadline.
    #[error("health check deadline exceeded")]
    DeadlineExceeded,
}

/// Stable query and value-decoding failure reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FdbQueryErrorReason {
    /// A transaction operation failed.
    #[error("transaction failed")]
    TransactionFailed,
    /// A FoundationDB tuple could not be decoded.
    #[error("tuple decode failed")]
    TupleDecodeFailed,
    /// A tuple did not match the required domain shape.
    #[error("tuple shape invalid")]
    TupleShapeInvalid,
    /// A stored value could not be decoded.
    #[error("value decode failed")]
    ValueDecodeFailed,
    /// Checked arithmetic or numeric conversion overflowed.
    #[error("integer overflow")]
    IntegerOverflow,
    /// A requested page size was zero or outside policy.
    #[error("page size invalid")]
    InvalidPageSize,
    /// A prefix could not produce a bounded exclusive range end.
    #[error("range prefix invalid")]
    InvalidRangePrefix,
    /// A tenant metadata key could not be constructed.
    #[error("metadata key construction invalid")]
    MetadataKeyInvalid,
}

/// Result type returned by FoundationDB kit operations.
pub type FdbResult<T> = Result<T, FdbError>;

/// Validates a non-zero page size and returns it as a typed value.
pub fn non_zero_page_size(value: usize) -> FdbResult<NonZeroUsize> {
    NonZeroUsize::new(value).ok_or(FdbError::Query {
        reason: FdbQueryErrorReason::InvalidPageSize,
    })
}
