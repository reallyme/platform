// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{S3ObjectKey, S3StorageError, S3StorageErrorReason};

/// Maximum body accepted by a single buffered object upload.
pub const MAX_S3_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

pub(crate) fn validate_upload(content_type: &str, body: &[u8]) -> Result<(), S3StorageError> {
    if body.len() > MAX_S3_UPLOAD_BYTES {
        return Err(S3StorageError::new(S3StorageErrorReason::ObjectTooLarge));
    }
    if body.is_empty()
        || content_type.trim().is_empty()
        || content_type.len() > 256
        || !content_type
            .bytes()
            .all(|b| b == b'\t' || (b' '..=b'~').contains(&b))
    {
        return Err(S3StorageError::new(S3StorageErrorReason::InvalidRequest));
    }
    Ok(())
}

/// Typed put-object upload request.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct S3PutObjectRequest {
    #[zeroize(skip)]
    object_key: S3ObjectKey,
    content_type: String,
    body: Vec<u8>,
}

impl S3PutObjectRequest {
    /// Constructs a validated object upload request.
    pub fn new(
        object_key: S3ObjectKey,
        content_type: String,
        body: Vec<u8>,
    ) -> Result<Self, S3StorageError> {
        let mut body = Zeroizing::new(body);
        validate_upload(&content_type, &body)?;

        Ok(Self {
            object_key,
            content_type: content_type.trim().to_owned(),
            body: std::mem::take(&mut *body),
        })
    }

    /// Returns the destination object key.
    pub fn object_key(&self) -> &S3ObjectKey {
        &self.object_key
    }

    /// Returns the upload content type.
    pub fn content_type(&self) -> &str {
        self.content_type.as_str()
    }

    /// Returns the upload body bytes.
    pub fn body(&self) -> &[u8] {
        self.body.as_slice()
    }
}

impl std::fmt::Debug for S3PutObjectRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3PutObjectRequest")
            .field("body", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
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
}
