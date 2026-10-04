// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use zeroize::Zeroizing;

use crate::{S3StorageError, S3StorageErrorReason};

/// Extends a bounded secret buffer without letting Vec free an uncleared
/// prior allocation during growth.
pub(crate) fn append_sensitive_chunk(
    body: &mut Zeroizing<Vec<u8>>,
    chunk: &[u8],
    maximum_bytes: usize,
) -> Result<(), S3StorageError> {
    let new_length = body
        .len()
        .checked_add(chunk.len())
        .ok_or_else(|| S3StorageError::new(S3StorageErrorReason::ObjectTooLarge))?;
    if new_length > maximum_bytes {
        return Err(S3StorageError::new(S3StorageErrorReason::ObjectTooLarge));
    }
    if body.capacity() < new_length {
        let doubled = body.capacity().checked_mul(2).unwrap_or(maximum_bytes);
        let capacity = doubled.min(maximum_bytes).max(new_length);
        let mut replacement = Zeroizing::new(Vec::new());
        replacement
            .try_reserve_exact(capacity)
            .map_err(|_| S3StorageError::new(S3StorageErrorReason::DownloadUnavailable))?;
        replacement.extend_from_slice(body.as_slice());
        let old = std::mem::replace(body, replacement);
        drop(old);
    }
    body.extend_from_slice(chunk);
    Ok(())
}

#[cfg(test)]
#[path = "bounded_body_tests.rs"]
mod tests;
