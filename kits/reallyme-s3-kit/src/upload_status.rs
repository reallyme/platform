// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use super::upload_status;
    use crate::S3StorageErrorReason;
    #[test]
    fn conflict_is_not_evidence_of_an_existing_object() {
        assert!(matches!(upload_status(200), Ok(true)));
        assert!(matches!(upload_status(412), Ok(false)));
        for status in [301, 307, 400, 403, 409, 500] {
            assert!(matches!(
                upload_status(status),
                Err(error) if error.reason == S3StorageErrorReason::UploadUnavailable
            ));
        }
    }
}
