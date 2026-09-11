// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use super::S3ObjectKey;

    #[test]
    fn object_key_accepts_expected_segments() {
        let result = S3ObjectKey::new(String::from("hephaestus/logs/node-1/report.json"));
        assert!(result.is_ok());
        let key = match result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(key.as_str(), "hephaestus/logs/node-1/report.json");
    }

    #[test]
    fn object_key_rejects_parent_segments() {
        assert!(S3ObjectKey::new(String::from("../unsafe")).is_err());
    }
}
