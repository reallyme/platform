// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hardened client construction and bounded request execution.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use reqwest::{Body, Client, header};
use zeroize::Zeroizing;

use super::{
    BoundedHttpsRequest, BoundedHttpsResponse, HttpsOrigin, HttpsTransportError,
    HttpsTransportErrorReason, response::read_response,
};

/// Reusable HTTPS client pinned to one validated origin.
#[derive(Clone)]
pub struct BoundedHttpsClient {
    origin: HttpsOrigin,
    client: Client,
}

impl BoundedHttpsClient {
    /// Builds a client that refuses plaintext HTTP and never follows redirects.
    pub fn new(origin: HttpsOrigin) -> Result<Self, HttpsTransportError> {
        let client = hardened_builder()?.build().map_err(|_| {
            HttpsTransportError::local(HttpsTransportErrorReason::ClientInitializationFailed)
        })?;
        Ok(Self { origin, client })
    }

    /// Executes a bounded exchange without retaining request or response content.
    pub async fn execute(
        &self,
        request: BoundedHttpsRequest<'_>,
    ) -> Result<BoundedHttpsResponse, HttpsTransportError> {
        let url = self.origin.resolve(request.target)?;
        let body = zeroizing_body(request.body)?;
        let mut builder = self
            .client
            .request(request.method.as_reqwest(), url)
            .timeout(request.timeout)
            .header(header::ACCEPT_ENCODING, "identity")
            .body(body);
        if let Some(content_type) = request.content_type {
            builder = builder.header(header::CONTENT_TYPE, content_type);
        }
        if let Some(accept) = request.accept {
            builder = builder.header(header::ACCEPT, accept);
        }
        let built = builder.build().map_err(|_| {
            HttpsTransportError::local(HttpsTransportErrorReason::RequestConstructionFailed)
        })?;
        let response = self.client.execute(built).await.map_err(|_| {
            // Once execution starts, connection and timeout errors cannot prove
            // that the peer did not receive and act on the request.
            HttpsTransportError::dispatched(HttpsTransportErrorReason::ExchangeFailed)
        })?;
        read_response(
            response,
            request.limits.maximum_response_bytes(),
            request.captured_response_header,
        )
        .await
    }
}

impl std::fmt::Debug for BoundedHttpsClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundedHttpsClient")
            .field("origin", &self.origin)
            .field("client", &"[configured]")
            .finish()
    }
}

fn zeroizing_body(body: &[u8]) -> Result<Body, HttpsTransportError> {
    let mut owned = Zeroizing::new(Vec::new());
    owned.try_reserve_exact(body.len()).map_err(|_| {
        HttpsTransportError::local(HttpsTransportErrorReason::RequestAllocationFailed)
    })?;
    owned.extend_from_slice(body);

    // `Bytes::from_owner` retains the zeroizing owner instead of copying into
    // an ordinary allocation. All clones used by the HTTP stack share it, and
    // the allocation is erased when the final clone is dropped.
    Ok(Body::from(Bytes::from_owner(owned)))
}

fn hardened_builder() -> Result<reqwest::ClientBuilder, HttpsTransportError> {
    const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
    const IDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
    const MAXIMUM_IDLE_CONNECTIONS_PER_HOST: usize = 8;

    let mut roots = rustls::RootCertStore::empty();
    let (accepted, _) =
        roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    if accepted == 0 {
        return Err(HttpsTransportError::local(
            HttpsTransportErrorReason::ClientInitializationFailed,
        ));
    }
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| HttpsTransportError::local(HttpsTransportErrorReason::ClientInitializationFailed))?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(Client::builder()
        .tls_backend_preconfigured(tls)
        // Credentials must not transit an ambient proxy chosen by process env.
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .https_only(true)
        .referer(false)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .connect_timeout(CONNECTION_TIMEOUT)
        .pool_idle_timeout(IDLE_CONNECTION_TIMEOUT)
        .pool_max_idle_per_host(MAXIMUM_IDLE_CONNECTIONS_PER_HOST))
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
