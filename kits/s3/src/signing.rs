// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_crypto::core::MacAlgorithm;
use reallyme_crypto::hmac::{HmacKey, authenticate};
use reallyme_crypto::sha2;
use secrecy::ExposeSecret;
use time::{OffsetDateTime, macros::format_description};
use url::Url;
use zeroize::Zeroizing;

use crate::{S3ObjectKey, S3StorageConfig, S3StorageError, S3StorageErrorReason};

pub(crate) const AWS_ALGORITHM: &str = "AWS4-HMAC-SHA256";
pub(crate) const AWS_REQUEST_TYPE: &str = "aws4_request";
const AWS_SERVICE: &str = "s3";
const HEADER_HOST: &str = "host";
const HEADER_IF_NONE_MATCH: &str = "if-none-match";
const HEADER_X_AMZ_CONTENT_SHA256: &str = "x-amz-content-sha256";
const HEADER_X_AMZ_DATE: &str = "x-amz-date";
const PUT_IF_ABSENT_SIGNED_HEADERS: &str = "host;if-none-match;x-amz-content-sha256;x-amz-date";
const GET_SIGNED_HEADERS: &str = "host;x-amz-content-sha256;x-amz-date";

/// SHA-256 digest of an empty request body, required by SigV4 GET requests.
pub const EMPTY_SHA256_HEX: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// Supported immutable S3 operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum S3SignedMethod {
    /// Load one immutable ciphertext object.
    Get,
    /// Test whether one immutable ciphertext object exists.
    Head,
    /// Insert one object only when the key does not already exist.
    PutIfAbsent,
    /// Delete one immutable ciphertext object after coordinator authorization.
    Delete,
}

impl S3SignedMethod {
    const fn as_http_method(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::PutIfAbsent => "PUT",
            Self::Delete => "DELETE",
        }
    }

    const fn signed_headers(self) -> &'static str {
        match self {
            Self::Get | Self::Head | Self::Delete => GET_SIGNED_HEADERS,
            Self::PutIfAbsent => PUT_IF_ABSENT_SIGNED_HEADERS,
        }
    }

    /// Returns whether the request must carry `If-None-Match: *`.
    #[must_use]
    pub const fn requires_if_none_match(self) -> bool {
        matches!(self, Self::PutIfAbsent)
    }
}

/// Fully signed request metadata for a S3 transport adapter.
pub struct S3SignedRequest {
    object_url: Url,
    authorization: Zeroizing<String>,
    amz_date: String,
    payload_hash: String,
    method: S3SignedMethod,
}

impl S3SignedRequest {
    /// Returns the exact allowlisted object URL covered by the signature.
    #[must_use]
    pub const fn object_url(&self) -> &Url {
        &self.object_url
    }

    /// Returns the SigV4 Authorization header value.
    #[must_use]
    pub fn authorization(&self) -> &str {
        self.authorization.as_str()
    }

    /// Returns the `x-amz-date` header value.
    #[must_use]
    pub fn amz_date(&self) -> &str {
        self.amz_date.as_str()
    }

    /// Returns the `x-amz-content-sha256` header value.
    #[must_use]
    pub fn payload_hash(&self) -> &str {
        self.payload_hash.as_str()
    }

    /// Returns the signed operation.
    #[must_use]
    pub const fn method(&self) -> S3SignedMethod {
        self.method
    }
}

impl std::fmt::Debug for S3SignedRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3SignedRequest")
            .field("object_url", &self.object_url)
            .field("authorization", &"<redacted>")
            .field("amz_date", &self.amz_date)
            .field("payload_hash", &self.payload_hash)
            .field("method", &self.method)
            .finish()
    }
}

/// Signs one bounded immutable-object operation using AWS Signature Version 4.
pub fn sign_object_request(
    config: &S3StorageConfig,
    method: S3SignedMethod,
    object_key: &S3ObjectKey,
    body: &[u8],
    timestamp: OffsetDateTime,
) -> Result<S3SignedRequest, S3StorageError> {
    if !matches!(method, S3SignedMethod::PutIfAbsent) && !body.is_empty() {
        return Err(S3StorageError::new(S3StorageErrorReason::InvalidRequest));
    }

    let object_url = object_url(config, object_key)?;
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
    let payload_hash = if body.is_empty() {
        String::from(EMPTY_SHA256_HEX)
    } else {
        sha256_hex(body)
    };
    let canonical_uri = canonical_uri(object_url.path())?;
    let host_header = canonical_host_header(&object_url)?;
    let canonical_headers = canonical_headers(
        method,
        host_header.as_str(),
        payload_hash.as_str(),
        amz_date.as_str(),
    );
    let canonical_request = canonical_request(
        method,
        canonical_uri.as_str(),
        canonical_headers.as_str(),
        payload_hash.as_str(),
    );
    let credential_scope = credential_scope(short_date.as_str(), config.region());
    let string_to_sign = string_to_sign(
        amz_date.as_str(),
        credential_scope.as_str(),
        sha256_hex(canonical_request.as_bytes()).as_str(),
    );
    let signing_key = derive_signing_key(
        config.secret_access_key().expose_secret(),
        short_date.as_str(),
        config.region(),
    )?;
    let signature = hmac_hex(signing_key.as_ref(), string_to_sign.as_bytes())?;
    let authorization = authorization_header(
        config.access_key_id().expose_secret(),
        credential_scope.as_str(),
        method.signed_headers(),
        signature.as_str(),
    );

    Ok(S3SignedRequest {
        object_url,
        authorization: Zeroizing::new(authorization),
        amz_date,
        payload_hash,
        method,
    })
}

pub(crate) fn object_url(
    config: &S3StorageConfig,
    object_key: &S3ObjectKey,
) -> Result<Url, S3StorageError> {
    let mut endpoint = config.endpoint().clone();
    let path_length = config
        .bucket()
        .len()
        .checked_add(object_key.as_str().len())
        .and_then(|length| length.checked_add(2))
        .ok_or_else(|| S3StorageError::new(S3StorageErrorReason::InvalidRequest))?;
    let mut path = String::with_capacity(path_length);
    path.push('/');
    path.push_str(config.bucket());
    path.push('/');
    path.push_str(object_key.as_str());
    endpoint.set_path(path.as_str());
    endpoint.set_query(None);
    Ok(endpoint)
}

pub(crate) fn canonical_uri(path: &str) -> Result<String, S3StorageError> {
    if path.is_empty() || !path.starts_with('/') {
        return Err(S3StorageError::new(S3StorageErrorReason::SigningFailed));
    }
    Ok(path.to_owned())
}

pub(crate) fn canonical_host_header(url: &Url) -> Result<String, S3StorageError> {
    let host = url
        .host_str()
        .ok_or_else(|| S3StorageError::new(S3StorageErrorReason::SigningFailed))?;
    match url.port() {
        Some(port) => {
            let mut value = String::from(host);
            value.push(':');
            value.push_str(&port.to_string());
            Ok(value)
        }
        _ => Ok(String::from(host)),
    }
}

fn canonical_headers(
    method: S3SignedMethod,
    host: &str,
    payload_hash: &str,
    amz_date: &str,
) -> String {
    let mut value = String::new();
    value.push_str(HEADER_HOST);
    value.push(':');
    value.push_str(host);
    value.push('\n');
    if method.requires_if_none_match() {
        value.push_str(HEADER_IF_NONE_MATCH);
        value.push_str(":*\n");
    }
    value.push_str(HEADER_X_AMZ_CONTENT_SHA256);
    value.push(':');
    value.push_str(payload_hash);
    value.push('\n');
    value.push_str(HEADER_X_AMZ_DATE);
    value.push(':');
    value.push_str(amz_date);
    value.push('\n');
    value
}

fn canonical_request(
    method: S3SignedMethod,
    canonical_uri: &str,
    canonical_headers: &str,
    payload_hash: &str,
) -> String {
    let mut value = String::new();
    value.push_str(method.as_http_method());
    value.push('\n');
    value.push_str(canonical_uri);
    value.push_str("\n\n");
    value.push_str(canonical_headers);
    value.push('\n');
    value.push_str(method.signed_headers());
    value.push('\n');
    value.push_str(payload_hash);
    value
}

pub(crate) fn credential_scope(short_date: &str, region: &str) -> String {
    let mut value = String::new();
    value.push_str(short_date);
    value.push('/');
    value.push_str(region);
    value.push('/');
    value.push_str(AWS_SERVICE);
    value.push('/');
    value.push_str(AWS_REQUEST_TYPE);
    value
}

pub(crate) fn string_to_sign(
    amz_date: &str,
    credential_scope: &str,
    canonical_request_hash: &str,
) -> String {
    let mut value = String::new();
    value.push_str(AWS_ALGORITHM);
    value.push('\n');
    value.push_str(amz_date);
    value.push('\n');
    value.push_str(credential_scope);
    value.push('\n');
    value.push_str(canonical_request_hash);
    value
}

fn authorization_header(
    access_key_id: &str,
    credential_scope: &str,
    signed_headers: &str,
    signature: &str,
) -> String {
    let mut value = String::new();
    value.push_str(AWS_ALGORITHM);
    value.push_str(" Credential=");
    value.push_str(access_key_id);
    value.push('/');
    value.push_str(credential_scope);
    value.push_str(", SignedHeaders=");
    value.push_str(signed_headers);
    value.push_str(", Signature=");
    value.push_str(signature);
    value
}

pub(crate) fn derive_signing_key(
    secret_access_key: &str,
    short_date: &str,
    region: &str,
) -> Result<Zeroizing<[u8; 32]>, S3StorageError> {
    let mut k_secret = Zeroizing::new(String::from("AWS4"));
    k_secret.push_str(secret_access_key);
    let k_date = hmac_bytes(k_secret.as_bytes(), short_date.as_bytes())?;
    let k_region = hmac_bytes(k_date.as_slice(), region.as_bytes())?;
    let k_service = hmac_bytes(k_region.as_slice(), AWS_SERVICE.as_bytes())?;
    let k_signing = hmac_bytes(k_service.as_slice(), AWS_REQUEST_TYPE.as_bytes())?;
    let mut key = Zeroizing::new([0_u8; 32]);
    key.as_mut().copy_from_slice(k_signing.as_slice());
    Ok(key)
}

fn hmac_bytes(key: &[u8], message: &[u8]) -> Result<Zeroizing<Vec<u8>>, S3StorageError> {
    let hmac_key = HmacKey::from_slice(key)
        .map_err(|_| S3StorageError::new(S3StorageErrorReason::SigningFailed))?;
    let tag = authenticate(MacAlgorithm::HmacSha256, &hmac_key, message)
        .map_err(|_| S3StorageError::new(S3StorageErrorReason::SigningFailed))?;
    Ok(Zeroizing::new(tag.as_bytes().to_vec()))
}

pub(crate) fn hmac_hex(key: &[u8], message: &[u8]) -> Result<String, S3StorageError> {
    hmac_bytes(key, message).map(|bytes| hex_encode(bytes.as_slice()))
}

pub(crate) fn sha256_hex(payload: &[u8]) -> String {
    let digest = sha2::digest(payload);
    hex_encode(digest.as_bytes())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::new();
    for byte in bytes {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value
}

#[cfg(test)]
#[path = "signing_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "signing_regression_tests.rs"]
#[allow(clippy::expect_used)]
mod regression_tests;
