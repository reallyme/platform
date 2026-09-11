// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
#[allow(clippy::expect_used)]
mod tests {
    use super::{MAX_RESPONSE_BYTES, decode_json, read_body};
    use bytes::Bytes;
    use futures_util::stream;
    use reqwest::{Body, Response};
    use serde_json::Value;

    fn response(chunks: Vec<Vec<u8>>) -> Response {
        let body = Body::wrap_stream(stream::iter(
            chunks
                .into_iter()
                .map(|chunk| Ok::<Bytes, std::io::Error>(Bytes::from(chunk))),
        ));
        Response::from(http::Response::new(body))
    }

    #[tokio::test]
    async fn rejects_oversized_chunked_responses_without_a_length_header() {
        let body = response(vec![vec![b' '; MAX_RESPONSE_BYTES], vec![b'x']]);
        assert!(read_body(body).await.is_err());
        let body = response(vec![vec![b' '; MAX_RESPONSE_BYTES]]);
        assert_eq!(
            read_body(body).await.expect("exact bound").len(),
            MAX_RESPONSE_BYTES
        );
    }

    #[tokio::test]
    async fn rejects_oversized_known_body_length_before_reading() {
        let body = http::Response::builder()
            .header("content-length", MAX_RESPONSE_BYTES + 1)
            .body(Body::from(vec![b' '; MAX_RESPONSE_BYTES + 1]))
            .expect("response");
        assert!(read_body(Response::from(body)).await.is_err());
    }

    #[tokio::test]
    async fn bounded_json_preserves_valid_data_and_rejects_malicious_nesting() {
        let value: Value = decode_json(response(vec![b"{\"ok\":true}".to_vec()]))
            .await
            .expect("JSON");
        assert_eq!(value["ok"], true);
        let nested = format!("{}0{}", "[".repeat(200), "]".repeat(200));
        assert!(
            decode_json::<Value>(response(vec![nested.into_bytes()]))
                .await
                .is_err()
        );
        assert!(
            decode_json::<Value>(response(vec![vec![0xff]]))
                .await
                .is_err()
        );
    }
}
