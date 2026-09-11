// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typesense connector construction.

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

        if config.use_gzip() {
            builder = builder.gzip(true);
        }

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
    /// The API key could not be represented as a safe HTTP header.
    InvalidApiKeyHeader,
    /// The reqwest client builder failed.
    HttpClientBuildFailed,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::TypesenseConnector;
    use crate::typesense::{TypesenseConfig, TypesenseEndpoint, TypesenseError};
    use secrecy::SecretString;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn excessive_retry_after_returns_rate_limit_without_retrying_early() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let config = TypesenseConfig::new(
            TypesenseEndpoint::parse(format!("http://{address}")).expect("endpoint"),
            SecretString::from("key"),
            Duration::from_secs(1),
        )
        .expect("config")
        .with_max_retries(2)
        .with_retry_max_delay(Duration::from_millis(5));
        let connector = TypesenseConnector::connect(config).expect("connector");
        let serve = async {
            let (mut stream, _) = listener.accept().await.expect("request");
            let mut buffer = [0u8; 4096];
            let received = stream.read(&mut buffer).await.expect("read");
            assert!(
                received > 0,
                "request must arrive before the fixture responds"
            );
            stream.write_all(b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 18446744073709551615\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await.expect("response");
        };
        let request = async {
            assert!(matches!(connector.health_report().await,
                Err(TypesenseError::Upstream { status, .. }) if status == 429));
        };
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(serve, request);
        })
        .await
        .expect("bounded response");
        assert!(
            tokio::time::timeout(Duration::from_millis(30), listener.accept())
                .await
                .is_err(),
            "must not issue a premature retry"
        );
    }

    #[tokio::test]
    async fn redirects_cannot_forward_the_typesense_api_key() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let destination = TcpListener::bind("127.0.0.1:0").await.expect("destination");
        let address = listener.local_addr().expect("address");
        let target = destination.local_addr().expect("target");
        let config = TypesenseConfig::new(
            TypesenseEndpoint::parse(format!("http://{address}")).expect("endpoint"),
            SecretString::from("secret-test-api-key"),
            Duration::from_secs(1),
        )
        .expect("config")
        .with_max_retries(0);
        let connector = TypesenseConnector::connect(config).expect("connector");
        let serve = async {
            let (mut stream, _) = listener.accept().await.expect("request");
            let mut buffer = [0u8; 4096];
            let length = stream.read(&mut buffer).await.expect("read");
            assert!(
                std::str::from_utf8(&buffer[..length])
                    .expect("headers")
                    .contains("secret-test-api-key")
            );
            let response = format!(
                "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{target}/health\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("response");
        };
        let request = async {
            assert!(matches!(connector.health_report().await,
                Err(TypesenseError::Upstream { status, .. }) if status == 307));
        };
        tokio::time::timeout(Duration::from_secs(3), async {
            tokio::join!(serve, request);
        })
        .await
        .expect("bounded test");
        assert!(
            tokio::time::timeout(Duration::from_millis(20), destination.accept())
                .await
                .is_err()
        );
    }
}
