// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    MAX_TENANT_KEY_BYTES, TenantDataAccessErrorReason, TenantDataKey, TenantDataRange,
    TenantDataRangeLimit, validate_atomic_mutation,
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
