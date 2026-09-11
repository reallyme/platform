// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Typed S3 storage error for audit-friendly callers.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
#[error("S3 storage error: {reason}")]
pub struct S3StorageError {
    /// Low-cardinality failure reason.
    pub reason: S3StorageErrorReason,
}

impl S3StorageError {
    /// Constructs a typed S3 storage error.
    pub const fn new(reason: S3StorageErrorReason) -> Self {
        Self { reason }
    }
}

/// Stable S3 storage failure reasons.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum S3StorageErrorReason {
    /// The endpoint or bucket configuration is invalid.
    #[error("invalid endpoint")]
    InvalidEndpoint,
    /// Required credentials or required fields were omitted.
    #[error("unconfigured")]
    Unconfigured,
    /// The configured key prefix is unsafe or malformed.
    #[error("invalid key prefix")]
    InvalidKeyPrefix,
    /// The requested object key is unsafe or malformed.
    #[error("invalid object key")]
    InvalidObjectKey,
    /// The caller supplied an invalid request body or content type.
    #[error("invalid request")]
    InvalidRequest,
    /// The upload could not be signed safely.
    #[error("signing failed")]
    SigningFailed,
    /// The HTTP client could not be constructed.
    #[error("client unavailable")]
    ClientUnavailable,
    /// The object upload failed or returned a non-success status.
    #[error("upload unavailable")]
    UploadUnavailable,
    /// The requested object could not be loaded safely.
    #[error("download unavailable")]
    DownloadUnavailable,
    /// The requested immutable object could not be deleted safely.
    #[error("delete unavailable")]
    DeleteUnavailable,
    /// The object already exists and immutable insertion was refused.
    #[error("object already exists")]
    ObjectAlreadyExists,
    /// The returned object exceeded the caller's configured bound.
    #[error("object too large")]
    ObjectTooLarge,
}
