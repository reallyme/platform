// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Query-authenticated GET capabilities for S3-compatible immutable objects.

use secrecy::ExposeSecret;
use time::{OffsetDateTime, macros::format_description};
use zeroize::Zeroizing;

use crate::signing::{
    AWS_ALGORITHM, canonical_host_header, canonical_uri, credential_scope, derive_signing_key,
    hmac_hex, object_url, sha256_hex, string_to_sign,
};
use crate::{S3ObjectKey, S3StorageConfig, S3StorageError, S3StorageErrorReason};

const PRESIGNED_SIGNED_HEADERS: &str = "host";
const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";

/// Maximum expiry accepted by AWS Signature Version 4 presigned requests.
pub const MAX_S3_PRESIGN_TTL_SECONDS: u32 = 604_800;

/// Sensitive bearer capability for one exact S3-compatible immutable-object GET.
pub struct S3PresignedGet {
    url: Zeroizing<String>,
}

impl S3PresignedGet {
    /// Borrows the complete HTTPS GET URL. Treat the returned value as a bearer secret.
    #[must_use]
    pub fn expose_url(&self) -> &str {
        self.url.as_str()
    }

    /// Transfers the URL into an owned zeroizing string.
    #[must_use]
    pub fn into_url(self) -> Zeroizing<String> {
        self.url
    }
}

impl core::fmt::Debug for S3PresignedGet {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("S3PresignedGet([REDACTED])")
    }
}

/// Issues a SigV4 query-authenticated GET for one exact object key.
///
/// The caller owns the shorter application authorization deadline. This primitive enforces the
/// provider maximum and never creates non-GET capabilities or accepts an unvalidated object key.
pub fn presign_get_object(
    config: &S3StorageConfig,
    object_key: &S3ObjectKey,
    expires_seconds: u32,
    timestamp: OffsetDateTime,
) -> Result<S3PresignedGet, S3StorageError> {
    if expires_seconds == 0 || expires_seconds > MAX_S3_PRESIGN_TTL_SECONDS {
        return Err(S3StorageError::new(S3StorageErrorReason::InvalidRequest));
    }

    let object_url = object_url(config, object_key)?;
    let canonical_path = canonical_uri(object_url.path())?;
    let host = canonical_host_header(&object_url)?;
    // SigV4 timestamps are UTC even when callers provide a nonzero offset.
    let timestamp = timestamp.to_offset(time::UtcOffset::UTC);
    let amz_date = timestamp
        .format(&format_description!(
            "[year][month][day]T[hour][minute][second]Z"
        ))
        .map_err(|_| S3StorageError::new(S3StorageErrorReason::SigningFailed))?;
    let short_date = timestamp
        .format(&format_description!("[year][month][day]"))
        .map_err(|_| S3StorageError::new(S3StorageErrorReason::SigningFailed))?;
    let scope = credential_scope(short_date.as_str(), config.region());
    let credential_length = config
        .access_key_id()
        .expose_secret()
        .len()
        .checked_add(1)
        .and_then(|value| value.checked_add(scope.len()))
        .ok_or_else(|| S3StorageError::new(S3StorageErrorReason::InvalidRequest))?;
    let mut credential = Zeroizing::new(String::with_capacity(credential_length));
    credential.push_str(config.access_key_id().expose_secret());
    credential.push('/');
    credential.push_str(scope.as_str());

    let encoded_credential = percent_encode(credential.as_bytes());
    let canonical_query = canonical_query(
        encoded_credential.as_str(),
        amz_date.as_str(),
        expires_seconds,
    );
    let canonical_request = canonical_presigned_request(
        canonical_path.as_str(),
        canonical_query.as_str(),
        host.as_str(),
    );
    let signing_key = derive_signing_key(
        config.secret_access_key().expose_secret(),
        short_date.as_str(),
        config.region(),
    )?;
    let signing_input = string_to_sign(
        amz_date.as_str(),
        scope.as_str(),
        sha256_hex(canonical_request.as_bytes()).as_str(),
    );
    let signature = Zeroizing::new(hmac_hex(signing_key.as_ref(), signing_input.as_bytes())?);
    let output_length = object_url
        .as_str()
        .len()
        .checked_add(1)
        .and_then(|value| value.checked_add(canonical_query.len()))
        .and_then(|value| value.checked_add("&X-Amz-Signature=".len()))
        .and_then(|value| value.checked_add(signature.len()))
        .ok_or_else(|| S3StorageError::new(S3StorageErrorReason::InvalidRequest))?;
    let mut output = Zeroizing::new(String::with_capacity(output_length));
    output.push_str(object_url.as_str());
    output.push('?');
    output.push_str(canonical_query.as_str());
    output.push_str("&X-Amz-Signature=");
    output.push_str(signature.as_str());
    Ok(S3PresignedGet { url: output })
}

fn canonical_query(credential: &str, amz_date: &str, expires_seconds: u32) -> Zeroizing<String> {
    let mut value = Zeroizing::new(String::new());
    value.push_str("X-Amz-Algorithm=");
    value.push_str(AWS_ALGORITHM);
    value.push_str("&X-Amz-Credential=");
    value.push_str(credential);
    value.push_str("&X-Amz-Date=");
    value.push_str(amz_date);
    value.push_str("&X-Amz-Expires=");
    value.push_str(expires_seconds.to_string().as_str());
    value.push_str("&X-Amz-SignedHeaders=");
    value.push_str(PRESIGNED_SIGNED_HEADERS);
    value
}

fn canonical_presigned_request(path: &str, query: &str, host: &str) -> Zeroizing<String> {
    let mut value = Zeroizing::new(String::from("GET\n"));
    value.push_str(path);
    value.push('\n');
    value.push_str(query);
    value.push_str("\nhost:");
    value.push_str(host);
    value.push_str("\n\nhost\n");
    value.push_str(UNSIGNED_PAYLOAD);
    value
}

fn percent_encode(bytes: &[u8]) -> Zeroizing<String> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut value = Zeroizing::new(String::new());
    for byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~') {
            value.push(char::from(*byte));
        } else {
            value.push('%');
            value.push(char::from(HEX[usize::from(byte >> 4)]));
            value.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    value
}

#[cfg(test)]
#[path = "presign_tests.rs"]
mod tests;
