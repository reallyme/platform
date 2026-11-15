// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;

use reallyme_foundationdb_kit::fdb::tenant::admin::reservations::{
    ReservationControlTenant, ReservationGeneration, ReservationId,
};
use reallyme_foundationdb_kit::{FoundationDbTenantName, fdb::tenant::admin::TenantAdminClusterId};

use super::{
    DeleteRecord, DeleteRecordError, DeleteRecordState, DeleteReservation, LockedDeleteRecord,
};

fn fixture_record() -> DeleteRecord {
    DeleteRecord {
        cluster: TenantAdminClusterId::from_bytes(b"0123456789abcdef0123456789abcdef")
            .expect("cluster UID fixture"),
        tenant: FoundationDbTenantName::new("catalog").expect("tenant fixture"),
        original_id: 42,
        state: DeleteRecordState::Pending,
        reservation: None,
    }
}

#[test]
fn creation_is_exclusive_and_completion_survives_reopening() {
    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("intent");
    let file = LockedDeleteRecord::open(&path).expect("acquire record lock");
    assert!(matches!(file.load(), Err(DeleteRecordError::Missing)));
    let record = fixture_record();
    file.create(record).expect("persist intent");
    assert!(matches!(
        LockedDeleteRecord::open(&path),
        Err(DeleteRecordError::Busy)
    ));
    assert_eq!(file.create(record), Err(DeleteRecordError::AlreadyExists));
    for reserved in ["intent.lock", "other.LOCK"] {
        assert!(matches!(
            LockedDeleteRecord::open(&directory.path().join(reserved)),
            Err(DeleteRecordError::InvalidPath),
        ));
    }
    file.complete(record, DeleteRecordState::MetadataSafe)
        .expect("persist completion");
    drop(file);
    let file = LockedDeleteRecord::open(&path).expect("reacquire record lock");
    let loaded = file.load().expect("reload record");
    assert!(loaded.cluster == record.cluster);
    assert_eq!(loaded.tenant, record.tenant);
    assert_eq!(loaded.original_id, record.original_id);
    assert!(loaded.state == DeleteRecordState::MetadataSafe);
    assert_eq!(
        file.complete(record, DeleteRecordState::OriginalAbsent),
        Err(DeleteRecordError::InvalidRecord)
    );
}

#[test]
fn rejects_invalid_or_oversized_records() {
    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("intent");
    let file = LockedDeleteRecord::open(&path).expect("acquire record lock");
    for contents in [
        b"".as_slice(),
        b"fdb-tenant-delete-v2\n0123456789abcdef0123456789abcdef\ncatalog\n42\npending\n",
        b"fdb-tenant-delete-v1\ninvalid\ncatalog\n42\npending\n",
        b"fdb-tenant-delete-v1\n0123456789abcdef0123456789abcdef\nInvalid\n42\npending\n",
        b"fdb-tenant-delete-v1\n0123456789abcdef0123456789abcdef\ncatalog\n-1\npending\n",
        b"fdb-tenant-delete-v1\n0123456789abcdef0123456789abcdef\ncatalog\n42\nunknown\n",
        b"fdb-tenant-delete-v1\n0123456789abcdef0123456789abcdef\ncatalog\n42\npending",
        &[b'a'; 321],
    ] {
        fs::write(&path, contents).expect("write invalid fixture");
        assert!(matches!(file.load(), Err(DeleteRecordError::InvalidRecord)));
    }
}

#[test]
fn reserved_record_round_trips_control_identity_and_operation() {
    let directory = tempfile::tempdir().expect("temporary state directory");
    let control_name =
        FoundationDbTenantName::new("operator-control").expect("fixture control name");
    for (name, generation) in [
        ("generation-zero", ReservationGeneration::LEGACY),
        ("stamped", ReservationGeneration::new(u64::MAX)),
    ] {
        let path = directory.path().join(name);
        let file = LockedDeleteRecord::open(&path).expect("lock record");
        let record = DeleteRecord {
            reservation: Some(DeleteReservation {
                control: ReservationControlTenant::new(control_name, 11)
                    .expect("fixture control ID"),
                operation: ReservationId::from_bytes([0xab; 16]),
                generation,
            }),
            ..fixture_record()
        };
        file.create(record).expect("write reserved record");
        let encoded = fs::read(&path).expect("read generated record bytes");
        assert!(
            DeleteRecord::decode_protobuf(&encoded).expect("decode generated record") == record
        );
        let mut invalid = encoded;
        invalid.extend_from_slice(&[0x38, 0x01]);
        assert!(DeleteRecord::decode_protobuf(&invalid).is_err());
        assert!(file.load().expect("read reserved record") == record);
        file.complete(record, DeleteRecordState::MetadataSafe)
            .expect("complete reserved record");
        assert!(
            file.load().expect("read completed record")
                == DeleteRecord {
                    state: DeleteRecordState::MetadataSafe,
                    ..record
                }
        );
    }
}

#[test]
fn reads_all_historical_versions_and_upgrades_on_completion() {
    let directory = tempfile::tempdir().expect("temporary state directory");
    let path = directory.path().join("intent");
    let file = LockedDeleteRecord::open(&path).expect("acquire record lock");
    for (version, reservation_fields, generation) in [
        ("v1", "", None),
        (
            "v2",
            "operator-control\n11\nabababababababababababababababab\n",
            Some(ReservationGeneration::LEGACY),
        ),
        (
            "v3",
            "operator-control\n11\nabababababababababababababababab\n7\n",
            Some(ReservationGeneration::new(7)),
        ),
    ] {
        let legacy = format!(
            "fdb-tenant-delete-{version}\n0123456789abcdef0123456789abcdef\ncatalog\n42\npending\n\
             {reservation_fields}"
        );
        fs::write(&path, legacy).expect("write historical record");
        let record = file.load().expect("read historical intent");
        assert!(record.state == DeleteRecordState::Pending);
        assert_eq!(
            record.reservation.map(|reserved| reserved.generation),
            generation
        );
        file.complete(record, DeleteRecordState::OriginalAbsent)
            .expect("persist the reconciled outcome");
        let encoded = fs::read(&path).expect("read upgraded record");
        assert!(
            DeleteRecord::decode_protobuf(&encoded).expect("decode upgraded file")
                == DeleteRecord {
                    state: DeleteRecordState::OriginalAbsent,
                    ..record
                }
        );
    }
}
