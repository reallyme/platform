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
    for (name, version, generation) in [
        (
            "legacy",
            "fdb-tenant-delete-v2",
            ReservationGeneration::LEGACY,
        ),
        (
            "stamped",
            "fdb-tenant-delete-v3",
            ReservationGeneration::new(u64::MAX),
        ),
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
        assert!(
            fs::read_to_string(&path)
                .expect("read record bytes")
                .starts_with(version)
        );
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
