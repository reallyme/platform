// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Core NATS connection construction and JetStream context setup.

use std::sync::Arc;
use std::time::Duration;

use async_nats::ConnectOptions;
use async_nats::jetstream::context::ContextBuilder;
use nkeys::KeyPair;
use secrecy::ExposeSecret;

use super::validation::validate_nats_url;
use super::{JetStreamCredentials, JetStreamTlsPolicy};
use crate::error::JetStreamError;

/// Connects a core NATS client for JetStream use.
pub async fn connect_client(nats_url: &str) -> Result<async_nats::Client, JetStreamError> {
    let tls_policy = JetStreamTlsPolicy::derive_from_url(nats_url)?;
    connect_with_credentials(nats_url, tls_policy, &JetStreamCredentials::None).await
}

/// Connects a core NATS client for JetStream use with explicit credentials.
pub async fn connect_with_credentials(
    nats_url: &str,
    tls_policy: JetStreamTlsPolicy,
    credentials: &JetStreamCredentials,
) -> Result<async_nats::Client, JetStreamError> {
    let trimmed = validate_nats_url(nats_url, true, tls_policy)?;
    let options = match credentials {
        JetStreamCredentials::None => ConnectOptions::new(),
        JetStreamCredentials::Token(token) => {
            ConnectOptions::with_token(token.expose_secret().to_owned())
        }
        JetStreamCredentials::Jwt { jwt, nkey_seed } => {
            let key_pair = Arc::new(
                KeyPair::from_seed(nkey_seed.expose_secret())
                    .map_err(|_| JetStreamError::InvalidConfiguration)?,
            );
            ConnectOptions::with_jwt(jwt.expose_secret().to_owned(), move |nonce| {
                let key_pair = Arc::clone(&key_pair);
                async move { key_pair.sign(&nonce).map_err(async_nats::AuthError::new) }
            })
        }
        JetStreamCredentials::NKey(seed) => {
            ConnectOptions::with_nkey(seed.expose_secret().to_owned())
        }
        JetStreamCredentials::CredentialsFile(path) => ConnectOptions::with_credentials_file(path)
            .await
            .map_err(|_| JetStreamError::ConnectFailed)?,
    };

    // async-nats otherwise calls rustls' process-default builder, which can
    // panic when a binary links more than one crypto provider.
    let tls = nats_tls_config()?;
    options
        .tls_client_config(tls)
        .require_tls(matches!(
            tls_policy,
            JetStreamTlsPolicy::Required | JetStreamTlsPolicy::Optional
        ))
        .connect(trimmed.as_str())
        .await
        .map_err(|_| JetStreamError::ConnectFailed)
}

fn nats_tls_config() -> Result<rustls::ClientConfig, JetStreamError> {
    let native = rustls_native_certs::load_native_certs();
    let mut roots = rustls::RootCertStore::empty();
    // A loopback plaintext connection needs no trust anchors. For TLS,
    // rustls fails closed at handshake if no usable roots were installed.
    let _ = roots.add_parsable_certificates(native.certs);
    build_nats_tls_config(roots)
}

fn build_nats_tls_config(
    roots: rustls::RootCertStore,
) -> Result<rustls::ClientConfig, JetStreamError> {
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map(|builder| builder.with_root_certificates(roots).with_no_client_auth())
        .map_err(|_| JetStreamError::ConnectFailed)
}

/// Connects a core NATS client for JetStream use with explicit credentials.
///
/// This name is kept for backwards compatibility.
pub async fn connect_client_with_credentials(
    nats_url: &str,
    tls_policy: JetStreamTlsPolicy,
    credentials: &JetStreamCredentials,
) -> Result<async_nats::Client, JetStreamError> {
    connect_with_credentials(nats_url, tls_policy, credentials).await
}

/// Creates a JetStream context with explicit request and ack timeouts.
pub fn create_context(
    client: async_nats::Client,
    operation_timeout: Duration,
    ack_timeout: Duration,
) -> async_nats::jetstream::Context {
    ContextBuilder::new()
        .timeout(operation_timeout)
        .ack_timeout(ack_timeout)
        .build(client)
}

#[cfg(test)]
mod tests {
    #[test]
    fn explicit_ring_provider_builds_with_reqwest_kit_in_graph() {
        let _ = std::any::type_name::<reallyme_typesense_kit::TypesenseConnector>();
        let result = super::build_nats_tls_config(rustls::RootCertStore::empty());
        assert!(result.is_ok());
    }
}
