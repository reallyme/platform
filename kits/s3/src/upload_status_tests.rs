// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
