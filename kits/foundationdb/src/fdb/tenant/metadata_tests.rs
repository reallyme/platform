// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{parse_created_at, parse_schema_version};
use crate::fdb::error::{FdbError, TenantErrorReason};
use crate::fdb::tenant_name::FoundationDbTenantName;

#[test]
fn parse_schema_version_rejects_wrong_version() {
    let tenant = FoundationDbTenantName::new("user").expect("test tenant should be valid");
    let result = parse_schema_version(tenant, &2_u32.to_le_bytes());
    assert!(matches!(
        result,
        Err(FdbError::Tenant {
            reason: TenantErrorReason::SchemaVersionUnsupported { .. }
        })
    ));
}

#[test]
fn parse_created_at_rejects_short_value() {
    let tenant = FoundationDbTenantName::new("user").expect("test tenant should be valid");
    let result = parse_created_at(tenant, &[0_u8; 4]);
    assert!(matches!(
        result,
        Err(FdbError::Tenant {
            reason: TenantErrorReason::MetadataMalformed { .. }
        })
    ));
}
