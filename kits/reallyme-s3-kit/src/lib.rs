// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! Reusable S3-compatible object storage primitives for ReallyMe services.
//!
//! This kit owns configuration validation, deterministic object-key validation,
//! SigV4 signing, and bounded object uploads. App-specific serialization,
//! object naming policy, and port contracts live outside this crate.

#[cfg(feature = "native-client")]
mod client;
mod config;
mod error;
mod object_key;
mod presign;
mod signing;
mod upload;
#[cfg(any(feature = "native-client", feature = "worker-client"))]
mod upload_status;
#[cfg(feature = "worker-client")]
mod worker_client;

#[cfg(feature = "native-client")]
pub use client::S3StorageClient;
pub use config::S3StorageConfig;
pub use error::{S3StorageError, S3StorageErrorReason};
pub use object_key::S3ObjectKey;
pub use presign::{MAX_S3_PRESIGN_TTL_SECONDS, S3PresignedGet, presign_get_object};
pub use signing::{EMPTY_SHA256_HEX, S3SignedMethod, S3SignedRequest, sign_object_request};
pub use upload::{MAX_S3_UPLOAD_BYTES, S3PutObjectRequest};
#[cfg(feature = "worker-client")]
pub use worker_client::WorkerS3StorageClient;
