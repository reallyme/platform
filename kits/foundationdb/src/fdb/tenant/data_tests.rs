// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    MAX_TENANT_KEY_BYTES, TenantDataAccessErrorReason, TenantDataKey, TenantDataRange,
    TenantDataRangeLimit, TenantDataRangeTargetBytes, TenantTransactionSizeLimit,
    validate_atomic_mutation, validate_versionstamped_key_template,
};
use foundationdb::options::MutationType;

#[test]
fn application_keys_exclude_kit_metadata_and_system_space() {
    for reserved in [
        b"__meta".as_slice(),
        b"__meta/v1/schema_version".as_slice(),
        b"__meta/v2/future_field".as_slice(),
        b"__meta_extra".as_slice(),
        b"\xff/management".as_slice(),
    ] {
        assert!(matches!(
            TenantDataKey::new(reserved),
            Err(TenantDataAccessErrorReason::ReservedKey)
        ));
    }
    for allowed in [
        b"app/item".as_slice(),
        b"__metb".as_slice(),
        b"\0item".as_slice(),
    ] {
        assert_eq!(
            TenantDataKey::new(allowed).map(TenantDataKey::as_bytes),
            Ok(allowed)
        );
    }
    assert!(matches!(
        TenantDataKey::new(b""),
        Err(TenantDataAccessErrorReason::EmptyKey)
    ));
    assert!(matches!(
        TenantDataKey::new(&vec![b'x'; MAX_TENANT_KEY_BYTES + 1]),
        Err(TenantDataAccessErrorReason::KeyTooLong)
    ));
}

#[test]
fn application_ranges_cannot_span_reserved_metadata() {
    for (begin, end) in [
        (b"__met".as_slice(), b"__metb".as_slice()),
        (b"__met".as_slice(), b"__metz".as_slice()),
        (b"_".as_slice(), b"z".as_slice()),
    ] {
        assert!(matches!(
            TenantDataRange::new(begin, end),
            Err(TenantDataAccessErrorReason::ReservedRange)
        ));
    }
    assert!(matches!(
        TenantDataRange::new(b"z", b"a"),
        Err(TenantDataAccessErrorReason::InvalidRange)
    ));
    assert!(matches!(
        TenantDataRange::new(b"x", b"x"),
        Err(TenantDataAccessErrorReason::InvalidRange)
    ));
    assert!(TenantDataRange::new(b"app/", b"app0").is_ok());
    assert!(TenantDataRange::new(b"__metb", b"__metc").is_ok());
}

#[test]
fn application_range_results_are_bounded() {
    assert!(matches!(
        TenantDataRangeLimit::new(0),
        Err(TenantDataAccessErrorReason::InvalidRangeLimit)
    ));
    assert!(matches!(
        TenantDataRangeLimit::new(65),
        Err(TenantDataAccessErrorReason::InvalidRangeLimit)
    ));
    assert_eq!(
        TenantDataRangeLimit::new(1).map(TenantDataRangeLimit::get),
        Ok(1)
    );
    assert_eq!(
        TenantDataRangeLimit::new(64).map(TenantDataRangeLimit::get),
        Ok(64)
    );
}

#[test]
fn transaction_and_range_byte_limits_reject_invalid_values() {
    assert!(matches!(
        TenantTransactionSizeLimit::new(0),
        Err(TenantDataAccessErrorReason::InvalidRangeLimit)
    ));
    assert!(matches!(
        TenantTransactionSizeLimit::new(10_000_001),
        Err(TenantDataAccessErrorReason::InvalidRangeLimit)
    ));
    assert!(TenantTransactionSizeLimit::new(900_000).is_ok());
    assert!(matches!(
        TenantDataRangeTargetBytes::new(0),
        Err(TenantDataAccessErrorReason::InvalidRangeLimit)
    ));
    assert!(matches!(
        TenantDataRangeTargetBytes::new(1_000_001),
        Err(TenantDataAccessErrorReason::InvalidRangeLimit)
    ));
    assert!(TenantDataRangeTargetBytes::new(800_000).is_ok());
}

#[test]
fn atomic_mutations_cannot_change_validated_keys() {
    assert!(matches!(
        validate_atomic_mutation(MutationType::SetVersionstampedKey),
        Err(TenantDataAccessErrorReason::KeyChangingMutation)
    ));
    for value_mutation in [
        MutationType::Add,
        MutationType::And,
        MutationType::BitAnd,
        MutationType::Or,
        MutationType::BitOr,
        MutationType::Xor,
        MutationType::BitXor,
        MutationType::AppendIfFits,
        MutationType::Max,
        MutationType::Min,
        MutationType::SetVersionstampedValue,
        MutationType::ByteMin,
        MutationType::ByteMax,
        MutationType::CompareAndClear,
    ] {
        assert!(validate_atomic_mutation(value_mutation).is_ok());
    }
}

#[test]
fn versionstamped_keys_keep_a_stable_application_prefix() {
    let mut valid = b"app/delivery/".to_vec();
    valid.extend_from_slice(&[0xff; 10]);
    valid.extend_from_slice(&7_u16.to_be_bytes());
    valid.extend_from_slice(&13_u32.to_le_bytes());
    assert!(validate_versionstamped_key_template(&valid).is_ok());

    let mut prefix_mutation = valid.clone();
    let prefix_offset = 1_u32.to_le_bytes();
    let offset_start = prefix_mutation.len() - prefix_offset.len();
    prefix_mutation[offset_start..].copy_from_slice(&prefix_offset);
    assert!(matches!(
        validate_versionstamped_key_template(&prefix_mutation),
        Err(TenantDataAccessErrorReason::KeyChangingMutation)
    ));

    let mut missing_placeholder = valid.clone();
    missing_placeholder[13] = 0;
    assert!(matches!(
        validate_versionstamped_key_template(&missing_placeholder),
        Err(TenantDataAccessErrorReason::KeyChangingMutation)
    ));

    let mut reserved = b"__meta/delivery/".to_vec();
    reserved.extend_from_slice(&[0xff; 10]);
    reserved.extend_from_slice(&7_u16.to_be_bytes());
    reserved.extend_from_slice(&16_u32.to_le_bytes());
    assert!(matches!(
        validate_versionstamped_key_template(&reserved),
        Err(TenantDataAccessErrorReason::ReservedKey)
    ));
}
