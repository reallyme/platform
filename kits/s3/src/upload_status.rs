// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{S3StorageError, S3StorageErrorReason};

pub(crate) fn upload_status(status: u16) -> Result<bool, S3StorageError> {
    match status {
        200..=299 => Ok(true),
        412 => Ok(false),
        // A concurrent delete can yield 409 without leaving any object behind.
        _ => Err(S3StorageError::new(S3StorageErrorReason::UploadUnavailable)),
    }
}

#[cfg(test)]
#[path = "upload_status_tests.rs"]
mod tests;
