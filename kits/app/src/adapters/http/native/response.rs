// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Zeroizing response data returned by the bounded HTTPS transport.

use std::fmt;

use reqwest::{Response, header::CONTENT_TYPE};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::{CapturedResponseHeader, HttpsTransportError, HttpsTransportErrorReason};

const MAXIMUM_CONTENT_TYPE_BYTES: usize = 256;

/// Status and bounded response fields whose owned bytes are erased on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct BoundedHttpsResponse {
    status_code: u16,
    content_type: Option<Vec<u8>>,
    captured_header: Option<Vec<u8>>,
    body: Vec<u8>,
}

impl BoundedHttpsResponse {
    /// Returns the HTTP status code without assigning application semantics.
    #[must_use]
    pub const fn status_code(&self) -> u16 {
        self.status_code
    }

    /// Returns the bounded `Content-Type` bytes, if supplied by the peer.
    #[must_use]
    pub fn content_type(&self) -> Option<&[u8]> {
        self.content_type.as_deref()
    }

    /// Returns the selected bounded response-header bytes, if present.
    #[must_use]
    pub fn captured_header(&self) -> Option<&[u8]> {
        self.captured_header.as_deref()
    }

    /// Returns the bounded response body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl fmt::Debug for BoundedHttpsResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundedHttpsResponse")
            .field("status_code", &self.status_code)
            .field("content_type", &self.content_type.as_ref().map(Vec::len))
            .field(
                "captured_header",
                &self.captured_header.as_ref().map(Vec::len),
            )
            .field(
                "body",
                &format_args!("[redacted; {} bytes]", self.body.len()),
            )
            .finish()
    }
}

pub(super) async fn read_response(
    mut response: Response,
    maximum_body_bytes: usize,
    captured_header: Option<CapturedResponseHeader>,
) -> Result<BoundedHttpsResponse, HttpsTransportError> {
    let maximum_body_u64 = u64::try_from(maximum_body_bytes).map_err(|_| {
        HttpsTransportError::dispatched(HttpsTransportErrorReason::ResponseLimitExceeded)
    })?;
    if response
        .content_length()
        .is_some_and(|length| length > maximum_body_u64)
    {
        return Err(HttpsTransportError::dispatched(
            HttpsTransportErrorReason::ResponseLimitExceeded,
        ));
    }

    let content_type = copy_header(
        response.headers().get(CONTENT_TYPE),
        MAXIMUM_CONTENT_TYPE_BYTES,
    )?;
    let captured_header = match captured_header {
        Some(selection) => {
            let name = selection
                .header_name()
                .map_err(|error| HttpsTransportError::dispatched(error.reason()))?;
            copy_header(response.headers().get(name), selection.maximum_bytes())?
        }
        None => None,
    };

    let mut body = Zeroizing::new(Vec::new());
    // Reserve the full caller-approved bound before copying any response byte.
    // Growing a sensitive Vec incrementally could leave an old allocation
    // unerased after reallocation; one bounded allocation avoids that remanence.
    body.try_reserve_exact(maximum_body_bytes).map_err(|_| {
        HttpsTransportError::dispatched(HttpsTransportErrorReason::ResponseAllocationFailed)
    })?;
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        HttpsTransportError::dispatched(HttpsTransportErrorReason::ResponseReadFailed)
    })? {
        let new_length = body.len().checked_add(chunk.len()).ok_or_else(|| {
            HttpsTransportError::dispatched(HttpsTransportErrorReason::ResponseLimitExceeded)
        })?;
        if new_length > maximum_body_bytes {
            return Err(HttpsTransportError::dispatched(
                HttpsTransportErrorReason::ResponseLimitExceeded,
            ));
        }
        body.extend_from_slice(&chunk);
    }

    Ok(BoundedHttpsResponse {
        status_code: response.status().as_u16(),
        content_type,
        captured_header,
        body: std::mem::take(&mut *body),
    })
}

fn copy_header(
    value: Option<&reqwest::header::HeaderValue>,
    maximum_bytes: usize,
) -> Result<Option<Vec<u8>>, HttpsTransportError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.as_bytes().len() > maximum_bytes {
        return Err(HttpsTransportError::dispatched(
            HttpsTransportErrorReason::ResponseHeaderLimitExceeded,
        ));
    }
    let mut copy = Zeroizing::new(Vec::new());
    copy.try_reserve_exact(value.as_bytes().len())
        .map_err(|_| {
            HttpsTransportError::dispatched(HttpsTransportErrorReason::ResponseAllocationFailed)
        })?;
    copy.extend_from_slice(value.as_bytes());
    Ok(Some(std::mem::take(&mut *copy)))
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
