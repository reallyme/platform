// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use std::time::Duration;
use time::OffsetDateTime;
use zeroize::Zeroizing;

use crate::{
    S3PutObjectRequest, S3SignedMethod, S3StorageConfig, S3StorageError, S3StorageErrorReason,
    sign_object_request,
};

const HEADER_X_AMZ_CONTENT_SHA256: &str = "x-amz-content-sha256";
const HEADER_X_AMZ_DATE: &str = "x-amz-date";

/// Minimal S3-compatible object storage client.
#[derive(Clone)]
pub struct S3StorageClient {
    http_client: Client,
    config: S3StorageConfig,
}

impl S3StorageClient {
    /// Constructs the reusable S3 storage client.
    pub fn new(config: S3StorageConfig) -> Result<Self, S3StorageError> {
        let http_client = http_client_builder()
            .build()
            .map_err(|_| S3StorageError::new(S3StorageErrorReason::ClientUnavailable))?;
        Ok(Self {
            http_client,
            config,
        })
    }

    /// Uploads a bounded object to S3-compatible storage with SigV4 request signing.
    pub async fn put_object(&self, request: S3PutObjectRequest) -> Result<(), S3StorageError> {
        let inserted = self
            .put_object_if_absent(
                request.object_key().as_str(),
                request.content_type(),
                request.body(),
            )
            .await?;
        if !inserted {
            return Err(S3StorageError::new(
                S3StorageErrorReason::ObjectAlreadyExists,
            ));
        }
        Ok(())
    }

    /// Loads one immutable object while enforcing a caller-selected response bound.
    pub async fn get_object(
        &self,
        relative_key: &str,
        maximum_bytes: usize,
    ) -> Result<Option<Vec<u8>>, S3StorageError> {
        if maximum_bytes == 0 {
            return Err(S3StorageError::new(S3StorageErrorReason::InvalidRequest));
        }
        let object_key = self.config.scoped_object_key(relative_key)?;
        let signed = sign_object_request(
            &self.config,
            S3SignedMethod::Get,
            &object_key,
            &[],
            OffsetDateTime::now_utc(),
        )?;
        let response = self
            .http_client
            .get(signed.object_url().clone())
            .header(reqwest::header::AUTHORIZATION, signed.authorization())
            .header(HEADER_X_AMZ_CONTENT_SHA256, signed.payload_hash())
            .header(HEADER_X_AMZ_DATE, signed.amz_date())
            .send()
            .await
            .map_err(|_| S3StorageError::new(S3StorageErrorReason::DownloadUnavailable))?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let maximum_bytes_u64 = u64::try_from(maximum_bytes)
            .map_err(|_| S3StorageError::new(S3StorageErrorReason::InvalidRequest))?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|length| length > maximum_bytes_u64)
        {
            return Err(S3StorageError::new(
                S3StorageErrorReason::DownloadUnavailable,
            ));
        }

        let mut stream = response.bytes_stream();
        let mut body = Zeroizing::new(Vec::new());
        while let Some(next) = stream.next().await {
            let chunk =
                next.map_err(|_| S3StorageError::new(S3StorageErrorReason::DownloadUnavailable))?;
            let next_length = body
                .len()
                .checked_add(chunk.len())
                .ok_or_else(|| S3StorageError::new(S3StorageErrorReason::ObjectTooLarge))?;
            if next_length > maximum_bytes {
                return Err(S3StorageError::new(S3StorageErrorReason::ObjectTooLarge));
            }
            body.extend_from_slice(chunk.as_ref());
        }
        Ok(Some(std::mem::take(&mut *body)))
    }

    /// Deletes one immutable object idempotently after coordinator authorization.
    ///
    /// This transport does not decide whether deletion is allowed. The owning
    /// application must issue and persist the garbage-collection claim first.
    pub async fn delete_object(&self, relative_key: &str) -> Result<bool, S3StorageError> {
        let object_key = self.config.scoped_object_key(relative_key)?;
        if !self.object_exists(&object_key).await? {
            return Ok(false);
        }
        let signed = sign_object_request(
            &self.config,
            S3SignedMethod::Delete,
            &object_key,
            &[],
            OffsetDateTime::now_utc(),
        )?;
        let response = self
            .http_client
            .delete(signed.object_url().clone())
            .header(reqwest::header::AUTHORIZATION, signed.authorization())
            .header(HEADER_X_AMZ_CONTENT_SHA256, signed.payload_hash())
            .header(HEADER_X_AMZ_DATE, signed.amz_date())
            .send()
            .await
            .map_err(|_| S3StorageError::new(S3StorageErrorReason::DeleteUnavailable))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if !response.status().is_success() {
            return Err(S3StorageError::new(S3StorageErrorReason::DeleteUnavailable));
        }
        Ok(true)
    }

    async fn object_exists(&self, object_key: &crate::S3ObjectKey) -> Result<bool, S3StorageError> {
        let signed = sign_object_request(
            &self.config,
            S3SignedMethod::Head,
            object_key,
            &[],
            OffsetDateTime::now_utc(),
        )?;
        let response = self
            .http_client
            .head(signed.object_url().clone())
            .header(reqwest::header::AUTHORIZATION, signed.authorization())
            .header(HEADER_X_AMZ_CONTENT_SHA256, signed.payload_hash())
            .header(HEADER_X_AMZ_DATE, signed.amz_date())
            .send()
            .await
            .map_err(|_| S3StorageError::new(S3StorageErrorReason::DeleteUnavailable))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if !response.status().is_success() {
            return Err(S3StorageError::new(S3StorageErrorReason::DeleteUnavailable));
        }
        Ok(true)
    }

    /// Inserts one immutable object using a signed `If-None-Match: *` request.
    pub async fn put_object_if_absent(
        &self,
        relative_key: &str,
        content_type: &str,
        body: &[u8],
    ) -> Result<bool, S3StorageError> {
        crate::upload::validate_upload(content_type, body)?;
        let object_key = self.config.scoped_object_key(relative_key)?;
        let signed = sign_object_request(
            &self.config,
            S3SignedMethod::PutIfAbsent,
            &object_key,
            body,
            OffsetDateTime::now_utc(),
        )?;
        let response = self
            .http_client
            .put(signed.object_url().clone())
            .header(reqwest::header::AUTHORIZATION, signed.authorization())
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .header(reqwest::header::IF_NONE_MATCH, "*")
            .header(HEADER_X_AMZ_CONTENT_SHA256, signed.payload_hash())
            .header(HEADER_X_AMZ_DATE, signed.amz_date())
            .body(body.to_vec())
            .send()
            .await
            .map_err(|_| S3StorageError::new(S3StorageErrorReason::UploadUnavailable))?;

        crate::upload_status::upload_status(response.status().as_u16())
    }
}

impl std::fmt::Debug for S3StorageClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3StorageClient")
            .field("config", &self.config)
            .finish()
    }
}

fn http_client_builder() -> reqwest::ClientBuilder {
    // A signature authorizes one endpoint and method. Redirects can replay an
    // upload body elsewhere, and automatic decompression changes stored bytes.
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .https_only(true)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests;
