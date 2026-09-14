// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]

use futures_util::StreamExt;
use time::OffsetDateTime;
use worker::{Fetch, Headers, Method, Request, RequestInit, RequestRedirect};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    S3SignedMethod, S3StorageConfig, S3StorageError, S3StorageErrorReason, sign_object_request,
};

const HEADER_AUTHORIZATION: &str = "authorization";
const HEADER_CONTENT_LENGTH: &str = "content-length";
const HEADER_CONTENT_TYPE: &str = "content-type";
const HEADER_IF_NONE_MATCH: &str = "if-none-match";
const HEADER_X_AMZ_CONTENT_SHA256: &str = "x-amz-content-sha256";
const HEADER_X_AMZ_DATE: &str = "x-amz-date";
const STATUS_NOT_FOUND: u16 = 404;

/// S3-compatible client implemented with the Cloudflare Workers `fetch` runtime.
#[derive(Clone)]
pub struct WorkerS3StorageClient {
    config: S3StorageConfig,
}

impl WorkerS3StorageClient {
    /// Creates a Worker transport from already validated S3 configuration.
    #[must_use]
    pub const fn new(config: S3StorageConfig) -> Self {
        Self { config }
    }

    /// Loads one immutable object while enforcing a caller-selected response bound.
    pub async fn get_object(
        &self,
        relative_key: &str,
        maximum_bytes: usize,
    ) -> Result<Option<Vec<u8>>, S3StorageError> {
        if maximum_bytes == 0 {
            return Err(storage_error(S3StorageErrorReason::InvalidRequest));
        }
        let object_key = self.config.scoped_object_key(relative_key)?;
        let signed = sign_object_request(
            &self.config,
            S3SignedMethod::Get,
            &object_key,
            &[],
            worker_time()?,
        )?;
        let headers = signed_headers(&signed, None)?;
        let mut init = RequestInit::new();
        // Never forward signed credentials or object data to a redirect target.
        init.with_redirect(RequestRedirect::Manual);
        init.with_method(Method::Get).with_headers(headers);
        let request = Request::new_with_init(signed.object_url().as_str(), &init)
            .map_err(|_| storage_error(S3StorageErrorReason::DownloadUnavailable))?;
        let mut response = Fetch::Request(request)
            .send()
            .await
            .map_err(|_| storage_error(S3StorageErrorReason::DownloadUnavailable))?;
        if response.status_code() == STATUS_NOT_FOUND {
            return Ok(None);
        }
        if !(200..300).contains(&response.status_code()) {
            return Err(storage_error(S3StorageErrorReason::DownloadUnavailable));
        }
        enforce_content_length(response.headers(), maximum_bytes)?;
        let mut stream = response
            .stream()
            .map_err(|_| storage_error(S3StorageErrorReason::DownloadUnavailable))?;
        let mut body = Zeroizing::new(Vec::new());
        while let Some(next) = stream.next().await {
            let mut chunk =
                next.map_err(|_| storage_error(S3StorageErrorReason::DownloadUnavailable))?;
            let next_length = body
                .len()
                .checked_add(chunk.len())
                .ok_or_else(|| storage_error(S3StorageErrorReason::ObjectTooLarge))?;
            if next_length > maximum_bytes {
                chunk.zeroize();
                body.zeroize();
                return Err(storage_error(S3StorageErrorReason::ObjectTooLarge));
            }
            body.extend_from_slice(chunk.as_slice());
            chunk.zeroize();
        }
        Ok(Some(std::mem::take(&mut *body)))
    }

    /// Deletes one immutable object idempotently after coordinator authorization.
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
            worker_time()?,
        )?;
        let headers = signed_headers(&signed, None)?;
        let mut init = RequestInit::new();
        // Never forward signed credentials or object data to a redirect target.
        init.with_redirect(RequestRedirect::Manual);
        init.with_method(Method::Delete).with_headers(headers);
        let request = Request::new_with_init(signed.object_url().as_str(), &init)
            .map_err(|_| storage_error(S3StorageErrorReason::DeleteUnavailable))?;
        let response = Fetch::Request(request)
            .send()
            .await
            .map_err(|_| storage_error(S3StorageErrorReason::DeleteUnavailable))?;
        if response.status_code() == STATUS_NOT_FOUND {
            return Ok(false);
        }
        if !(200..300).contains(&response.status_code()) {
            return Err(storage_error(S3StorageErrorReason::DeleteUnavailable));
        }
        Ok(true)
    }

    async fn object_exists(&self, object_key: &crate::S3ObjectKey) -> Result<bool, S3StorageError> {
        let signed = sign_object_request(
            &self.config,
            S3SignedMethod::Head,
            object_key,
            &[],
            worker_time()?,
        )?;
        let headers = signed_headers(&signed, None)?;
        let mut init = RequestInit::new();
        // Never forward signed credentials or object data to a redirect target.
        init.with_redirect(RequestRedirect::Manual);
        init.with_method(Method::Head).with_headers(headers);
        let request = Request::new_with_init(signed.object_url().as_str(), &init)
            .map_err(|_| storage_error(S3StorageErrorReason::DeleteUnavailable))?;
        let response = Fetch::Request(request)
            .send()
            .await
            .map_err(|_| storage_error(S3StorageErrorReason::DeleteUnavailable))?;
        if response.status_code() == STATUS_NOT_FOUND {
            return Ok(false);
        }
        if !(200..300).contains(&response.status_code()) {
            return Err(storage_error(S3StorageErrorReason::DeleteUnavailable));
        }
        Ok(true)
    }

    /// Inserts immutable ciphertext with a signed `If-None-Match: *` request.
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
            worker_time()?,
        )?;
        let headers = signed_headers(&signed, Some(content_type))?;
        let bytes = worker::js_sys::Uint8Array::from(body);
        let mut init = RequestInit::new();
        // Never forward signed credentials or object data to a redirect target.
        init.with_redirect(RequestRedirect::Manual);
        init.with_method(Method::Put)
            .with_headers(headers)
            .with_body(Some(bytes.into()));
        let request = Request::new_with_init(signed.object_url().as_str(), &init)
            .map_err(|_| storage_error(S3StorageErrorReason::UploadUnavailable))?;
        let response = Fetch::Request(request)
            .send()
            .await
            .map_err(|_| storage_error(S3StorageErrorReason::UploadUnavailable))?;
        crate::upload_status::upload_status(response.status_code())
    }
}

impl core::fmt::Debug for WorkerS3StorageClient {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("WorkerS3StorageClient")
            .field("config", &self.config)
            .finish()
    }
}

fn signed_headers(
    signed: &crate::S3SignedRequest,
    content_type: Option<&str>,
) -> Result<Headers, S3StorageError> {
    let headers = Headers::new();
    headers
        .set(HEADER_AUTHORIZATION, signed.authorization())
        .and_then(|()| headers.set(HEADER_X_AMZ_CONTENT_SHA256, signed.payload_hash()))
        .and_then(|()| headers.set(HEADER_X_AMZ_DATE, signed.amz_date()))
        .map_err(|_| storage_error(S3StorageErrorReason::InvalidRequest))?;
    if let Some(value) = content_type {
        headers
            .set(HEADER_CONTENT_TYPE, value)
            .and_then(|()| headers.set(HEADER_IF_NONE_MATCH, "*"))
            .map_err(|_| storage_error(S3StorageErrorReason::InvalidRequest))?;
    }
    Ok(headers)
}

fn enforce_content_length(headers: &Headers, maximum_bytes: usize) -> Result<(), S3StorageError> {
    let value = headers
        .get(HEADER_CONTENT_LENGTH)
        .map_err(|_| storage_error(S3StorageErrorReason::DownloadUnavailable))?;
    validate_content_length(value.as_deref(), maximum_bytes)
}

// Keep untrusted header parsing independent of the Worker runtime so its
// fail-closed behavior remains testable on every supported build host.
fn validate_content_length(
    value: Option<&str>,
    maximum_bytes: usize,
) -> Result<(), S3StorageError> {
    let Some(value) = value else {
        return Ok(());
    };
    let parsed = value
        .parse::<u64>()
        .map_err(|_| storage_error(S3StorageErrorReason::DownloadUnavailable))?;
    let maximum = u64::try_from(maximum_bytes)
        .map_err(|_| storage_error(S3StorageErrorReason::InvalidRequest))?;
    if parsed > maximum {
        return Err(storage_error(S3StorageErrorReason::ObjectTooLarge));
    }
    Ok(())
}

fn worker_time() -> Result<OffsetDateTime, S3StorageError> {
    let millis = worker::Date::now().as_millis();
    let nanos = i128::from(millis)
        .checked_mul(1_000_000)
        .ok_or_else(|| storage_error(S3StorageErrorReason::InvalidRequest))?;
    OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .map_err(|_| storage_error(S3StorageErrorReason::InvalidRequest))
}

fn storage_error(reason: S3StorageErrorReason) -> S3StorageError {
    S3StorageError::new(reason)
}

#[cfg(test)]
#[path = "worker_client_tests.rs"]
mod tests;
