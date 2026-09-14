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

    match credentials {
        JetStreamCredentials::None => async_nats::connect(trimmed.as_str())
            .await
            .map_err(|_| JetStreamError::ConnectFailed),
        JetStreamCredentials::Token(token) => {
            { ConnectOptions::with_token(token.expose_secret().to_owned()) }
                .connect(trimmed.as_str())
                .await
                .map_err(|_| JetStreamError::ConnectFailed)
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
            .connect(trimmed.as_str())
            .await
            .map_err(|_| JetStreamError::ConnectFailed)
        }
        JetStreamCredentials::NKey(seed) => {
            { ConnectOptions::with_nkey(seed.expose_secret().to_owned()) }
                .connect(trimmed.as_str())
                .await
                .map_err(|_| JetStreamError::ConnectFailed)
        }
        JetStreamCredentials::CredentialsFile(path) => ConnectOptions::with_credentials_file(path)
            .await
            .map_err(|_| JetStreamError::ConnectFailed)?
            .connect(trimmed.as_str())
            .await
            .map_err(|_| JetStreamError::ConnectFailed),
    }
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
