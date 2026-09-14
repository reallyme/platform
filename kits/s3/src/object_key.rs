// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{S3StorageError, S3StorageErrorReason};

const MAX_OBJECT_KEY_BYTES: usize = 1_024;

/// Validated S3 object key.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct S3ObjectKey(String);

impl S3ObjectKey {
    /// Constructs a validated object key from caller-controlled input.
    pub fn new(value: String) -> Result<Self, S3StorageError> {
        let trimmed = value.trim().trim_matches('/').to_owned();
        if trimmed.is_empty() || trimmed.len() > MAX_OBJECT_KEY_BYTES {
            return Err(S3StorageError::new(S3StorageErrorReason::InvalidObjectKey));
        }
        if !trimmed
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.'))
        {
            return Err(S3StorageError::new(S3StorageErrorReason::InvalidObjectKey));
        }
        if trimmed
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        {
            return Err(S3StorageError::new(S3StorageErrorReason::InvalidObjectKey));
        }
        Ok(Self(trimmed))
    }

    /// Returns the validated object key as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Debug for S3ObjectKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("S3ObjectKey").field(&self.0).finish()
    }
}

#[cfg(test)]
#[path = "object_key_tests.rs"]
mod tests;
