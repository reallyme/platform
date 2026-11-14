// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;

use reallyme_foundationdb_kit::{FoundationDbTenantName, fdb::tenant::admin::TenantAdminClusterId};

use super::{DeleteRecord, DeleteRecordError, DeleteRecordState, LockedDeleteRecord};

fn fixture_record() -> DeleteRecord {
    DeleteRecord {
        cluster: TenantAdminClusterId::from_bytes(b"0123456789abcdef0123456789abcdef")
            .expect("cluster UID fixture"),
        tenant: FoundationDbTenantName::new("catalog").expect("tenant fixture"),
        original_id: 42,
        state: DeleteRecordState::Pending,
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
        &[b'a'; 257],
    ] {
        fs::write(&path, contents).expect("write invalid fixture");
        assert!(matches!(file.load(), Err(DeleteRecordError::InvalidRecord)));
    }
}
