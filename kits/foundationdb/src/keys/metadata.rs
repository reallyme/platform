// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tenant metadata keys owned by foundationdb-kit.

use bytes::{Bytes, BytesMut};

use super::codec::KeyEncoder;
use super::error::KeyLabelError;
use super::{namespace::KeyNamespace, prefix::KeyPrefix};

/// Stable metadata namespace within each tenant.
pub const TENANT_METADATA_NAMESPACE: &str = "__meta";
const TENANT_METADATA_VERSION_PREFIX: &str = "v1";
const SCHEMA_VERSION_KEY: &str = "schema_version";
const CREATED_AT_KEY: &str = "created_at";

/// Tenant-scoped metadata key for schema-version.
pub struct TenantMetadataSchemaVersionKey {
    namespace: KeyNamespace,
    prefix: KeyPrefix,
}

/// Tenant-scoped metadata key for metadata creation timestamp.
pub struct TenantMetadataCreatedAtKey {
    namespace: KeyNamespace,
    prefix: KeyPrefix,
}

/// Shared key prefix under `__meta`.
pub struct TenantMetadataVersionPrefix;

impl TenantMetadataVersionPrefix {
    /// Returns a validated `__meta` namespace/prefix pair.
    pub fn components() -> Result<(KeyNamespace, KeyPrefix), KeyLabelError> {
        Ok((
            KeyNamespace::from_static(TENANT_METADATA_NAMESPACE)?,
            KeyPrefix::from_static(TENANT_METADATA_VERSION_PREFIX)?,
        ))
    }
}

impl TenantMetadataSchemaVersionKey {
    /// Builds the `__meta/schema_version` key encoder for a version prefix.
    pub fn new() -> Result<Self, KeyLabelError> {
        let (namespace, prefix) = TenantMetadataVersionPrefix::components()?;
        Ok(Self { namespace, prefix })
    }
}

impl TenantMetadataCreatedAtKey {
    /// Builds the `__meta/created_at` key encoder for a version prefix.
    pub fn new() -> Result<Self, KeyLabelError> {
        let (namespace, prefix) = TenantMetadataVersionPrefix::components()?;
        Ok(Self { namespace, prefix })
    }
}

fn encode_key(namespace: &str, prefix: &str, key: &str) -> Bytes {
    let mut bytes = BytesMut::with_capacity(
        namespace
            .len()
            .checked_add(prefix.len())
            .and_then(|value| value.checked_add(key.len()))
            .and_then(|value| value.checked_add(2))
            .unwrap_or(namespace.len()),
    );

    bytes.extend_from_slice(namespace.as_bytes());
    bytes.extend_from_slice(b"/");
    bytes.extend_from_slice(prefix.as_bytes());
    bytes.extend_from_slice(b"/");
    bytes.extend_from_slice(key.as_bytes());

    bytes.freeze()
}

impl KeyEncoder for TenantMetadataSchemaVersionKey {
    fn encode_key(&self) -> Bytes {
        encode_key(
            self.namespace.as_str(),
            self.prefix.as_str(),
            SCHEMA_VERSION_KEY,
        )
    }
}

impl KeyEncoder for TenantMetadataCreatedAtKey {
    fn encode_key(&self) -> Bytes {
        encode_key(
            self.namespace.as_str(),
            self.prefix.as_str(),
            CREATED_AT_KEY,
        )
    }
}
