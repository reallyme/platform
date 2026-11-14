// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Durable, cluster-scoped intent for a single operator delete.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use reallyme_foundationdb_kit::{FoundationDbTenantName, fdb::tenant::admin::TenantAdminClusterId};
use tempfile::NamedTempFile;
use thiserror::Error;

const RECORD_VERSION: &str = "fdb-tenant-delete-v1";
const MAX_RECORD_BYTES: u64 = 256;

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DeleteRecordState {
    Pending,
    OriginalAbsent,
    MetadataSafe,
}

impl DeleteRecordState {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::OriginalAbsent => "original-absent",
            Self::MetadataSafe => "metadata-safe",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct DeleteRecord {
    pub(super) cluster: TenantAdminClusterId,
    pub(super) tenant: FoundationDbTenantName,
    pub(super) original_id: i64,
    pub(super) state: DeleteRecordState,
}

impl DeleteRecord {
    fn parse(bytes: &[u8]) -> Result<Self, DeleteRecordError> {
        let invalid = DeleteRecordError::InvalidRecord;
        let text = std::str::from_utf8(bytes).map_err(|_| invalid)?;
        let mut fields = text.strip_suffix('\n').ok_or(invalid)?.split('\n');
        if fields.next() != Some(RECORD_VERSION) {
            return Err(invalid);
        }
        let cluster = TenantAdminClusterId::from_bytes(fields.next().ok_or(invalid)?.as_bytes())
            .ok_or(invalid)?;
        let tenant =
            FoundationDbTenantName::new(fields.next().ok_or(invalid)?).map_err(|_| invalid)?;
        let id = fields.next().ok_or(invalid)?;
        if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid);
        }
        let original_id = id.parse::<i64>().map_err(|_| invalid)?;
        let state = match fields.next() {
            Some("pending") => DeleteRecordState::Pending,
            Some("original-absent") => DeleteRecordState::OriginalAbsent,
            Some("metadata-safe") => DeleteRecordState::MetadataSafe,
            _ => return Err(invalid),
        };
        if fields.next().is_some() {
            return Err(invalid);
        }
        Ok(Self {
            cluster,
            tenant,
            original_id,
            state,
        })
    }

    fn write(self, file: &mut File) -> Result<(), DeleteRecordError> {
        let uid = std::str::from_utf8(self.cluster.as_bytes())
            .map_err(|_| DeleteRecordError::InvalidRecord)?;
        writeln!(
            file,
            "{}\n{}\n{}\n{}\n{}",
            RECORD_VERSION,
            uid,
            self.tenant,
            self.original_id,
            self.state.as_str()
        )
        .map_err(|_| DeleteRecordError::PersistFailed)
    }
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
        if !metadata.is_file() || metadata.len() > MAX_RECORD_BYTES {
            return Err(DeleteRecordError::InvalidRecord);
        }
        let mut bytes = Vec::new();
        File::open(&self.path)
            .map_err(|_| DeleteRecordError::ReadFailed)?
            .take(
                MAX_RECORD_BYTES
                    .checked_add(1)
                    .ok_or(DeleteRecordError::ReadFailed)?,
            )
            .read_to_end(&mut bytes)
            .map_err(|_| DeleteRecordError::ReadFailed)?;
        if u64::try_from(bytes.len()).map_err(|_| DeleteRecordError::InvalidRecord)?
            > MAX_RECORD_BYTES
        {
            return Err(DeleteRecordError::InvalidRecord);
        }
        DeleteRecord::parse(&bytes)
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
        record.write(file.as_file_mut())?;
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
