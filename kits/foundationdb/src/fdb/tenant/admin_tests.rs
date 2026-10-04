// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::fdb::tenant_name::FoundationDbTenantName;
use crate::keys::{TenantMetadataSchemaVersionKey, codec::KeyEncoder};

use super::{
    MAX_TENANT_MAP_VALUE_BYTES, decode_tenant_prefix, prefixed_metadata_key, tenant_map_key,
};

#[test]
fn tenant_delete_keys_follow_fdb_v73_mapping_and_kit_metadata_encoding() {
    let tenant = FoundationDbTenantName::new("example").expect("valid tenant");
    assert_eq!(
        tenant_map_key(tenant).expect("map key"),
        b"\xff\xff/management/tenant/map/example"
    );
    let prefix = decode_tenant_prefix(br#"{"prefix":{"base64":"AQIDBA=="}}"#, tenant)
        .expect("valid FDB tenant prefix");
    assert_eq!(prefix, [1, 2, 3, 4]);
    let metadata_key = TenantMetadataSchemaVersionKey::new()
        .expect("metadata key")
        .encode_key();
    let raw =
        prefixed_metadata_key(&prefix, metadata_key.as_ref(), tenant).expect("raw metadata key");
    assert_eq!(raw.as_slice(), b"\x01\x02\x03\x04__meta/v1/schema_version");
}

#[test]
fn malformed_tenant_mapping_cannot_select_a_key_to_clear() {
    let tenant = FoundationDbTenantName::new("example").expect("valid tenant");
    for value in [
        br#"{"prefix":{"base64":"!"}}"#.as_slice(),
        br#"{"prefix":{"base64":""}}"#.as_slice(),
        br#"{"prefix":{}}"#.as_slice(),
        b"not-json".as_slice(),
    ] {
        assert!(decode_tenant_prefix(value, tenant).is_err());
    }
    assert!(decode_tenant_prefix(&vec![b'x'; MAX_TENANT_MAP_VALUE_BYTES + 1], tenant).is_err());
}
