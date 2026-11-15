// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Durable, cluster-scoped intent for a single operator delete.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub(super) use reallyme_foundationdb_kit::fdb::tenant::admin::delete_intent::{
    DeleteRecord, DeleteRecordState, DeleteReservation,
};
use reallyme_foundationdb_kit::{
    FoundationDbTenantName,
    fdb::tenant::admin::{
        TenantAdminClusterId,
        delete_intent::MAX_DELETE_RECORD_BYTES,
        reservations::{ReservationControlTenant, ReservationGeneration, ReservationId},
    },
};
use tempfile::NamedTempFile;
use thiserror::Error;
use zeroize::Zeroizing;

const LOCAL_RECORD: &str = "fdb-tenant-delete-v1";
const RESERVED_RECORD: &str = "fdb-tenant-delete-v2";
const STAMPED_RECORD: &str = "fdb-tenant-delete-v3";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(super) enum DeleteRecordError {
    #[error("delete record path is invalid")]
    InvalidPath,
    #[error("delete record or directory could not be opened")]
    OpenFailed,
    #[error("a delete or resume already holds the record lock")]
    Busy,
    #[error("delete record lock failed")]
    LockFailed,
    #[error("delete record already exists; use resume-delete")]
    AlreadyExists,
    #[error("delete record is missing")]
    Missing,
    #[error("delete record is invalid")]
    InvalidRecord,
    #[error("delete record could not be read")]
    ReadFailed,
    #[error("delete record could not be made durable")]
    PersistFailed,
}

fn parse_legacy(bytes: &[u8]) -> Result<DeleteRecord, DeleteRecordError> {
    let invalid = DeleteRecordError::InvalidRecord;
    let text = std::str::from_utf8(bytes).map_err(|_| invalid)?;
    let mut fields = text.strip_suffix('\n').ok_or(invalid)?.split('\n');
    let version = fields.next().ok_or(invalid)?;
    let cluster = TenantAdminClusterId::from_bytes(fields.next().ok_or(invalid)?.as_bytes())
        .ok_or(invalid)?;
    let tenant = FoundationDbTenantName::new(fields.next().ok_or(invalid)?).map_err(|_| invalid)?;
    let original_id = parse_id(fields.next().ok_or(invalid)?)?;
    let state = match fields.next() {
        Some("pending") => DeleteRecordState::Pending,
        Some("original-absent") => DeleteRecordState::OriginalAbsent,
        Some("metadata-safe") => DeleteRecordState::MetadataSafe,
        _ => return Err(invalid),
    };
    let reservation = if matches!(version, RESERVED_RECORD | STAMPED_RECORD) {
        let control =
            FoundationDbTenantName::new(fields.next().ok_or(invalid)?).map_err(|_| invalid)?;
        let id = parse_id(fields.next().ok_or(invalid)?)?;
        if control == tenant || id == original_id {
            return Err(invalid);
        }
        let control = ReservationControlTenant::new(control, id).map_err(|_| invalid)?;
        let operation = parse_operation(fields.next().ok_or(invalid)?)?;
        let generation = if version == STAMPED_RECORD {
            parse_generation(fields.next().ok_or(invalid)?).ok_or(invalid)?
        } else {
            ReservationGeneration::LEGACY
        };
        Some(DeleteReservation {
            control,
            operation,
            generation,
        })
    } else if version == LOCAL_RECORD {
        None
    } else {
        return Err(invalid);
    };
    if fields.next().is_some() {
        return Err(invalid);
    }
    Ok(DeleteRecord {
        cluster,
        tenant,
        original_id,
        state,
        reservation,
    })
}

pub(super) fn parse_generation(value: &str) -> Option<ReservationGeneration> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let value = value.parse::<u64>().ok()?;
    (value != 0).then_some(ReservationGeneration::new(value))
}

fn parse_id(value: &str) -> Result<i64, DeleteRecordError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(DeleteRecordError::InvalidRecord);
    }
    value
        .parse::<i64>()
        .map_err(|_| DeleteRecordError::InvalidRecord)
}

fn parse_operation(value: &str) -> Result<ReservationId, DeleteRecordError> {
    let invalid = DeleteRecordError::InvalidRecord;
    let mut id = Zeroizing::new([0; 16]);
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    if !remainder.is_empty()
        || pairs.len() != id.len()
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid);
    }
    for (output, pair) in id.iter_mut().zip(pairs) {
        let encoded = std::str::from_utf8(pair).map_err(|_| invalid)?;
        *output = u8::from_str_radix(encoded, 16).map_err(|_| invalid)?;
    }
    Ok(ReservationId::from_bytes(*id))
}

pub(super) struct LockedDeleteRecord {
    path: PathBuf,
    parent: PathBuf,
    directory: File,
    // The sidecar inode is stable across atomic replacement of the record.
    _lock: File,
}

impl LockedDeleteRecord {
    pub(super) fn open(path: &Path) -> Result<Self, DeleteRecordError> {
        let name = path.file_name().ok_or(DeleteRecordError::InvalidPath)?;
        // A record must never replace another record's stable lock inode.
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lock"))
        {
            return Err(DeleteRecordError::InvalidPath);
        }
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = fs::canonicalize(parent).map_err(|_| DeleteRecordError::OpenFailed)?;
        let directory = File::open(&parent).map_err(|_| DeleteRecordError::OpenFailed)?;
        if !directory
            .metadata()
            .map_err(|_| DeleteRecordError::OpenFailed)?
            .is_dir()
        {
            return Err(DeleteRecordError::InvalidPath);
        }
        let mut lock_name = name.to_os_string();
        lock_name.push(".lock");
        let lock_path = parent.join(lock_name);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match fs::symlink_metadata(&lock_path) {
            Ok(metadata) if !metadata.is_file() => return Err(DeleteRecordError::InvalidPath),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(DeleteRecordError::OpenFailed),
        }
        let lock = options
            .open(lock_path)
            .map_err(|_| DeleteRecordError::OpenFailed)?;
        if !lock
            .metadata()
            .map_err(|_| DeleteRecordError::OpenFailed)?
            .is_file()
        {
            return Err(DeleteRecordError::InvalidPath);
        }
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(DeleteRecordError::Busy),
            Err(TryLockError::Error(_)) => return Err(DeleteRecordError::LockFailed),
        }
        Ok(Self {
            path: parent.join(name),
            parent,
            directory,
            _lock: lock,
        })
    }

    pub(super) fn load(&self) -> Result<DeleteRecord, DeleteRecordError> {
        let metadata = fs::symlink_metadata(&self.path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                DeleteRecordError::Missing
            } else {
                DeleteRecordError::ReadFailed
            }
        })?;
        let length =
            usize::try_from(metadata.len()).map_err(|_| DeleteRecordError::InvalidRecord)?;
        if !metadata.is_file() || length > MAX_DELETE_RECORD_BYTES {
            return Err(DeleteRecordError::InvalidRecord);
        }
        let mut bytes = Zeroizing::new([0; MAX_DELETE_RECORD_BYTES]);
        let mut file = File::open(&self.path).map_err(|_| DeleteRecordError::ReadFailed)?;
        file.read_exact(&mut bytes[..length])
            .map_err(|_| DeleteRecordError::ReadFailed)?;
        // An external writer can grow the file after the metadata read. Reject
        // trailing bytes without allocating or accepting a partial record.
        let mut extra = Zeroizing::new([0]);
        match file.read_exact(&mut *extra) {
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {}
            Ok(()) => return Err(DeleteRecordError::InvalidRecord),
            Err(_) => return Err(DeleteRecordError::ReadFailed),
        }
        let payload = &bytes[..length];
        if payload.first() == Some(&0) {
            DeleteRecord::decode_protobuf(payload).map_err(|_| DeleteRecordError::InvalidRecord)
        } else {
            parse_legacy(payload)
        }
    }

    pub(super) fn create(&self, record: DeleteRecord) -> Result<(), DeleteRecordError> {
        let file = self.stage(record)?;
        file.persist_noclobber(&self.path).map_err(|failure| {
            if failure.error.kind() == io::ErrorKind::AlreadyExists {
                DeleteRecordError::AlreadyExists
            } else {
                DeleteRecordError::PersistFailed
            }
        })?;
        self.sync_directory()
    }

    pub(super) fn complete(
        &self,
        record: DeleteRecord,
        state: DeleteRecordState,
    ) -> Result<(), DeleteRecordError> {
        if record.state != DeleteRecordState::Pending
            || state == DeleteRecordState::Pending
            || self.load()? != record
        {
            return Err(DeleteRecordError::InvalidRecord);
        }
        let file = self.stage(DeleteRecord { state, ..record })?;
        file.persist(&self.path)
            .map_err(|_| DeleteRecordError::PersistFailed)?;
        self.sync_directory()
    }

    fn stage(&self, record: DeleteRecord) -> Result<NamedTempFile, DeleteRecordError> {
        let mut file =
            NamedTempFile::new_in(&self.parent).map_err(|_| DeleteRecordError::PersistFailed)?;
        let bytes = record
            .encode_protobuf()
            .map_err(|_| DeleteRecordError::InvalidRecord)?;
        file.write_all(&bytes)
            .map_err(|_| DeleteRecordError::PersistFailed)?;
        file.as_file()
            .sync_all()
            .map_err(|_| DeleteRecordError::PersistFailed)?;
        Ok(file)
    }

    pub(super) fn sync_directory(&self) -> Result<(), DeleteRecordError> {
        // A successful rename alone does not guarantee that its directory
        // entry survives power loss. Refuse to proceed if sync fails.
        self.directory
            .sync_all()
            .map_err(|_| DeleteRecordError::PersistFailed)
    }
}

#[cfg(test)]
#[path = "delete_record_tests.rs"]
mod tests;
