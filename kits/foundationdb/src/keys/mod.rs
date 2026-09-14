// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared key construction helpers.

pub mod codec;
pub mod error;
pub mod metadata;
pub mod namespace;
pub mod prefix;
pub(crate) mod validation;

pub use metadata::{
    TENANT_METADATA_NAMESPACE, TenantMetadataCreatedAtKey, TenantMetadataSchemaVersionKey,
    TenantMetadataVersionPrefix,
};
pub use namespace::KeyNamespace;
pub use prefix::KeyPrefix;
