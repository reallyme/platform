// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounds decompressed response bytes before parsing untrusted Typesense JSON.

use super::{TypesenseError, TypesenseResult, TypesenseTransportReason};
use reqwest::Response;
use serde::de::DeserializeOwned;
use zeroize::Zeroizing;

// Large enough for a full bulk response while preventing unbounded upstream
// allocations. Chunked and compressed responses must obey the same ceiling.
pub(super) const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

pub(super) async fn read_body(mut response: Response) -> TypesenseResult<Zeroizing<Vec<u8>>> {
    let maximum = u64::try_from(MAX_RESPONSE_BYTES).map_err(|_| invalid_body())?;
    if response
        .content_length()
        .is_some_and(|length| length > maximum)
    {
        return Err(invalid_body());
    }
    let mut body = Zeroizing::new(Vec::new());
    while let Some(chunk) = response.chunk().await.map_err(|_| invalid_body())? {
        let length = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(invalid_body)?;
        if length > MAX_RESPONSE_BYTES {
            return Err(invalid_body());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub(super) async fn decode_json<T: DeserializeOwned>(response: Response) -> TypesenseResult<T> {
    let body = read_body(response).await?;
    serde_json::from_slice(&body).map_err(|_| invalid_body())
}

fn invalid_body() -> TypesenseError {
    TypesenseError::Transport {
        reason: TypesenseTransportReason::InvalidResponseBody,
    }
}

#[cfg(test)]
#[path = "response_body_tests.rs"]
mod tests;
