// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typesense connector construction.

use std::sync::Arc;

use reqwest::{
    Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use secrecy::ExposeSecret;
use thiserror::Error;

use crate::typesense::{TypesenseClient, TypesenseConfig, TypesenseHealthReport, TypesenseResult};

const TYPESENSE_API_KEY_HEADER: HeaderName = HeaderName::from_static("x-typesense-api-key");

/// Owns the configured HTTP client used to talk to Typesense.
#[derive(Clone)]
pub struct TypesenseConnector {
    client: TypesenseClient,
}

impl TypesenseConnector {
    /// Builds a connector from validated configuration.
    pub fn connect(config: TypesenseConfig) -> Result<Self, ConnectorBuildError> {
        config
            .validate()
            .map_err(|_| ConnectorBuildError::Invalid {
                reason: ConnectorBuildErrorReason::InvalidRequestPolicy,
            })?;
        // safety: `ExposeSecret` is intentionally used once here to materialize the API
        // key header value. Typesense requires this key on every request, so we keep
        // it in default headers as a documented exception to header secret retention.
        let mut default_headers = HeaderMap::new();
        let mut api_key =
            HeaderValue::from_str(config.api_key().expose_secret()).map_err(|_| {
                ConnectorBuildError::Invalid {
                    reason: ConnectorBuildErrorReason::InvalidApiKeyHeader,
                }
            })?;
        api_key.set_sensitive(true);
        default_headers.insert(TYPESENSE_API_KEY_HEADER, api_key);

        let mut builder = Client::builder()
            .tls_backend_preconfigured(tls_config()?)
            // The API key must not transit a proxy inherited from process env.
            .no_proxy()
            // Custom credential headers are not stripped by redirect handling.
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(default_headers)
            .tcp_nodelay(true)
            .pool_max_idle_per_host(64);

        if let Some(pool_idle_timeout) = config.pool_idle_timeout() {
            builder = builder.pool_idle_timeout(pool_idle_timeout);
        }

        if let Some(tcp_keepalive) = config.tcp_keepalive() {
            builder = builder.tcp_keepalive(Some(tcp_keepalive));
        }

        if config.http2_prior_knowledge() {
            builder = builder.http2_prior_knowledge();
        }

        if config.http2_adaptive_window() {
            builder = builder.http2_adaptive_window(true);
        }

        if let Some(http2_keep_alive_interval) = config.http2_keep_alive_interval() {
            builder = builder
                .http2_keep_alive_interval(http2_keep_alive_interval)
                .http2_keep_alive_while_idle(true);
        }

        // The reqwest gzip feature enables automatic decompression by
        // default. Apply the caller's policy explicitly in both directions.
        builder = builder.gzip(config.use_gzip());

        let http_client = builder.build().map_err(|_| ConnectorBuildError::Invalid {
            reason: ConnectorBuildErrorReason::HttpClientBuildFailed,
        })?;

        Ok(Self {
            client: TypesenseClient::new(http_client, config),
        })
    }

    /// Returns the underlying Typesense client.
    #[must_use]
    pub const fn client(&self) -> &TypesenseClient {
        &self.client
    }

    /// Verifies that a configured Typesense node reports itself ready.
    pub async fn health_check(&self) -> TypesenseResult<()> {
        self.client.health_check().await
    }

    /// Returns bounded, non-sensitive readiness state from Typesense.
    pub async fn health_report(&self) -> TypesenseResult<TypesenseHealthReport> {
        self.client.health_report().await
    }
}

pub(crate) fn tls_config() -> Result<rustls::ClientConfig, ConnectorBuildError> {
    let mut roots = rustls::RootCertStore::empty();
    let _ = roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    if roots.is_empty() {
        // A host-managed trust store must not be widened by bundled roots.
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map(|builder| {
            let mut tls = builder.with_root_certificates(roots).with_no_client_auth();
            tls.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
            tls
        })
        .map_err(|_| ConnectorBuildError::Invalid {
            reason: ConnectorBuildErrorReason::HttpClientBuildFailed,
        })
}

/// Connector construction failure.
#[derive(Debug, Error)]
pub enum ConnectorBuildError {
    /// A connector could not be built from the supplied config.
    #[error("invalid typesense connector configuration")]
    Invalid {
        /// Stable machine-readable reason.
        reason: ConnectorBuildErrorReason,
    },
}

/// Stable connector construction failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorBuildErrorReason {
    /// A resource limit was invalid after configuration changes.
    InvalidRequestPolicy,
    /// The API key could not be represented as a safe HTTP header.
    InvalidApiKeyHeader,
    /// The reqwest client builder failed.
    HttpClientBuildFailed,
}

#[cfg(test)]
#[path = "connector_tests.rs"]
mod tests;
