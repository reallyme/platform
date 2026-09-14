// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_S3_UPLOAD_BYTES, validate_upload};
use crate::S3StorageErrorReason;
#[test]
fn uploads_reject_oversized_payloads_and_invalid_header_values() {
    assert!(matches!(
        validate_upload(
            "application/octet-stream",
            &vec![0; MAX_S3_UPLOAD_BYTES + 1]
        ),
        Err(error) if error.reason == S3StorageErrorReason::ObjectTooLarge
    ));
    for value in ["", "text/plain\r\nx-extra: injected", "\0"] {
        assert!(validate_upload(value, b"payload").is_err());
    }
    assert!(validate_upload("application/octet-stream", b"payload").is_ok());
    assert!(validate_upload("text/plain;\tcharset=utf-8", b"payload").is_ok());
}
