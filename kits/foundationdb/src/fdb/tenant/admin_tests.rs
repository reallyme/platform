// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::fdb::tenant_name::FoundationDbTenantName;

use super::metadata_keys;

#[test]
fn tenant_delete_targets_only_the_kits_two_metadata_keys() {
    let tenant = FoundationDbTenantName::new("example").expect("valid tenant");
    let [schema, created_at] = metadata_keys(tenant).expect("metadata keys");
    assert_eq!(schema.as_ref(), b"__meta/v1/schema_version");
    assert_eq!(created_at.as_ref(), b"__meta/v1/created_at");
}
