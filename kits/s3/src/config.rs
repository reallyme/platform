// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::{S3ObjectKey, S3StorageError, S3StorageErrorReason};

/// Typed S3-compatible object storage configuration.
#[derive(Clone)]
pub struct S3StorageConfig {
    endpoint: Url,
    region: String,
    bucket: String,
    access_key_id: SecretString,
    secret_access_key: SecretString,
    key_prefix: S3ObjectKey,
}

impl S3StorageConfig {
    /// Constructs validated S3-compatible storage configuration.
    pub fn new(
        endpoint: String,
        region: String,
        bucket: String,
        access_key_id: SecretString,
        secret_access_key: SecretString,
        key_prefix: String,
    ) -> Result<Self, S3StorageError> {
        if region.trim().is_empty()
            || bucket.trim().is_empty()
            || access_key_id.expose_secret().trim().is_empty()
            || secret_access_key.expose_secret().trim().is_empty()
        {
            return Err(S3StorageError::new(S3StorageErrorReason::Unconfigured));
        }

        // Bucket names are a single URL segment and regions enter the SigV4
        // credential scope. Reject separators and encoded traversal before signing.
        if !valid_bucket(bucket.trim()) || !valid_region(region.trim()) {
            return Err(S3StorageError::new(S3StorageErrorReason::InvalidEndpoint));
        }

        let endpoint = Url::parse(endpoint.trim())
            .map_err(|_| S3StorageError::new(S3StorageErrorReason::InvalidEndpoint))?;
        if !is_secure_s3_endpoint(&endpoint) {
            return Err(S3StorageError::new(S3StorageErrorReason::InvalidEndpoint));
        }

        let key_prefix = S3ObjectKey::new(key_prefix)
            .map_err(|_| S3StorageError::new(S3StorageErrorReason::InvalidKeyPrefix))?;

        Ok(Self {
            endpoint,
            region: region.trim().to_owned(),
            bucket: bucket.trim().to_owned(),
            access_key_id,
            secret_access_key,
            key_prefix,
        })
    }

    /// Returns the configured endpoint URL.
    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    /// Returns the configured SigV4 signing region.
    pub fn region(&self) -> &str {
        self.region.as_str()
    }

    /// Returns the configured bucket name.
    pub fn bucket(&self) -> &str {
        self.bucket.as_str()
    }

    /// Returns the configured access key identifier.
    pub fn access_key_id(&self) -> &SecretString {
        &self.access_key_id
    }

    /// Returns the configured secret access key.
    pub fn secret_access_key(&self) -> &SecretString {
        &self.secret_access_key
    }

    /// Returns the configured default object key prefix.
    pub fn key_prefix(&self) -> &S3ObjectKey {
        &self.key_prefix
    }

    /// Resolves a validated relative key beneath the configured immutable prefix.
    pub fn scoped_object_key(&self, relative_key: &str) -> Result<S3ObjectKey, S3StorageError> {
        let relative = S3ObjectKey::new(relative_key.to_owned())?;
        let joined_length = self
            .key_prefix
            .as_str()
            .len()
            .checked_add(1)
            .and_then(|length| length.checked_add(relative.as_str().len()))
            .ok_or_else(|| S3StorageError::new(S3StorageErrorReason::InvalidObjectKey))?;
        let mut joined = String::with_capacity(joined_length);
        joined.push_str(self.key_prefix.as_str());
        joined.push('/');
        joined.push_str(relative.as_str());
        S3ObjectKey::new(joined)
    }
}

fn is_secure_s3_endpoint(endpoint: &Url) -> bool {
    if endpoint.scheme() != "https"
        || endpoint.username() != ""
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || !matches!(endpoint.path(), "" | "/")
    {
        return false;
    }

    endpoint.host_str().is_some_and(|host| !host.is_empty())
}

impl std::fmt::Debug for S3StorageConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3StorageConfig")
            .field("endpoint", &self.endpoint.as_str())
            .field("region", &self.region)
            .field("bucket", &self.bucket)
            .field("access_key_id", &"<redacted>")
            .field("secret_access_key", &"<redacted>")
            .field("key_prefix", &self.key_prefix)
            .finish()
    }
}

// Historical S3 buckets allow 255 bytes, uppercase letters, and underscores.
// The kit targets existing S3-compatible buckets, not just new AWS buckets.
fn valid_bucket(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

fn valid_region(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
