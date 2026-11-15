// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_PERSISTED_RECORD_BYTES, PersistedRecordError, decode_record, encode_record};
use crate::generated::reallyme::platform::storage::v1::{CreationTime, SchemaVersion};

#[test]
fn scalar_records_are_tagged_canonical_and_bounded() {
    let version = SchemaVersion::default().with_version(1);
    let encoded = encode_record(&version);
    let decoded: SchemaVersion = decode_record(&encoded).expect("encoded schema version");
    assert_eq!(decoded.version, 1);
    assert!(encoded.len() > std::mem::size_of::<u32>());
    assert_eq!(
        decode_record::<CreationTime>(&encoded).err(),
        Some(PersistedRecordError::InvalidEncoding)
    );

    for suffix in [b"\x08\x01".as_slice(), b"\x10\x01", b"\xff"] {
        let invalid = [encoded.as_slice(), suffix].concat();
        assert_eq!(
            decode_record::<SchemaVersion>(&invalid).err(),
            Some(PersistedRecordError::InvalidEncoding)
        );
    }
    let mut oversized = encoded.to_vec();
    oversized.resize(MAX_PERSISTED_RECORD_BYTES + 1, 0);
    assert_eq!(
        decode_record::<SchemaVersion>(&oversized).err(),
        Some(PersistedRecordError::InvalidEncoding)
    );
}

#[cfg(feature = "tenant-admin")]
#[test]
fn reservation_nonce_uses_bounded_wiping_storage_across_fragments() {
    use buffa::{DecodeOptions, Message};
    use bytes::Buf;
    use zeroize::Zeroize;

    use crate::generated::reallyme::platform::storage::v1::{Reservation, ReservationPhase};
    use crate::stored_id::StoredId;

    let message = Reservation::default()
        .with_original_id(i64::MAX)
        .with_generation(u64::MAX)
        .with_epoch(u64::MAX)
        .with_claimant(StoredId::new(*b"opaque-test-id!!"))
        .with_expires_at(i64::MAX)
        .with_phase(ReservationPhase::Recovering);
    let bytes = message.encode_to_vec();
    let encoded = encode_record(&message);
    assert!(encoded.len() <= MAX_PERSISTED_RECORD_BYTES);
    for index in 1..bytes.len() {
        let (head, tail) = bytes.split_at(index);
        let decoded = Reservation::decode(&mut head.chain(tail)).expect("fragmented nonce decodes");
        assert_eq!(decoded.claimant.bytes(), message.claimant.bytes());
    }

    let mut clone = message.clone();
    assert!(!format!("{clone:?}").contains("opaque-test-id"));
    clone.zeroize();
    assert!(clone.claimant.bytes().is_none());
    assert_eq!(clone.epoch, 0);
    assert_eq!(message.claimant.bytes(), Some(b"opaque-test-id!!"));

    let mut destination = message.clone();
    let mut malformed = b"\x22\x02id".as_slice();
    let result = DecodeOptions::new()
        .reject_unknown_fields()
        .clear_on_merge_error()
        .merge(&mut destination, &mut malformed);
    assert!(result.is_err());
    assert!(destination.claimant.bytes().is_none());
    assert_eq!(destination.epoch, 0);
}

#[cfg(feature = "tenant-admin")]
#[test]
fn operator_cleanup_covers_nested_fields_and_rejects_unbounded_names() {
    use buffa::{DecodeOptions, Message, ProtoString};
    use bytes::Buf;
    use zeroize::{Zeroize, Zeroizing};

    use crate::generated::reallyme::platform::storage::v1::{
        OperatorDeleteRecord, OperatorDeleteReservation,
    };
    use crate::stored_id::{StoredClusterId, StoredId};
    use crate::stored_tenant_name::StoredTenantName;

    let mut message = OperatorDeleteRecord::default()
        .with_cluster_uid(StoredClusterId::new(*b"0123456789abcdef0123456789abcdef"))
        .with_tenant_name(StoredTenantName::copy_from_str("target-name"))
        .with_reservation(
            OperatorDeleteReservation::default()
                .with_control_name(StoredTenantName::copy_from_str("control-name"))
                .with_operation(StoredId::new(*b"opaque-test-id!!")),
        );
    let encoded = Zeroizing::new(message.encode_to_vec());
    for index in 1..encoded.len() {
        let (head, tail) = encoded.split_at(index);
        let decoded =
            OperatorDeleteRecord::decode(&mut head.chain(tail)).expect("fragmented record");
        assert_eq!(decoded, message);
    }
    let debug = format!("{message:?}");
    for value in [
        "0123456789abcdef",
        "target-name",
        "control-name",
        "opaque-test-id",
    ] {
        assert!(!debug.contains(value));
    }

    let mut nested = message
        .reservation
        .as_option()
        .expect("nested record")
        .clone();
    nested.zeroize();
    assert!(nested.control_name.is_empty());
    assert!(nested.operation.bytes().is_none());

    let invalid_name = b"\x12\x01\xff";
    let mut oversized = [b'a'; 66];
    oversized[..2].copy_from_slice(&[0x12, 64]);
    for invalid in [invalid_name.as_slice(), oversized.as_slice()] {
        let mut destination = message.clone();
        let error = DecodeOptions::new().clear_on_merge_error().merge(
            &mut destination,
            &mut invalid.split_at(1).0.chain(&invalid[1..]),
        );
        assert!(error.is_err());
        assert!(destination.reservation.as_option().is_none());
        assert!(destination.cluster_uid.bytes().is_none());
        assert!(destination.tenant_name.is_empty());
    }
    message.zeroize();
    assert!(message.reservation.as_option().is_none());
}
