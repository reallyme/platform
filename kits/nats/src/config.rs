// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::time::Duration;

use async_nats::jetstream::consumer::{DeliverPolicy, ReplayPolicy};
use secrecy::SecretString;

use crate::error::JetStreamError;

mod connection;
mod validation;

pub use connection::{
    connect_client, connect_client_with_credentials, connect_with_credentials, create_context,
};
use validation::{
    redact_nats_url, validate_component, validate_filter_subject, validate_nats_url,
    validate_publish_subject,
};

const MAX_NATS_URL_BYTES: usize = 2_048;
const MAX_STREAM_NAME_BYTES: usize = 255;
const MAX_CONSUMER_NAME_BYTES: usize = 255;
const MAX_SUBJECT_BYTES: usize = 255;
const LOCAL_SUBJECT_SUFFIX: &str = ".local";

/// TLS policy used to validate NATS connection URLs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum JetStreamTlsPolicy {
    /// Allow insecure connections for local development and explicit test environments.
    Disabled,
    /// Prefer TLS but allow cleartext transports if explicitly configured.
    #[default]
    Optional,
    /// Reject non-TLS transport URLs.
    Required,
}

/// Credentials for typed NATS authentication.
pub enum JetStreamCredentials {
    /// Connect without explicit credentials.
    None,
    /// Authenticate with a NATS token.
    Token(SecretString),
    /// Authenticate with a JWT and NKey signing key.
    Jwt {
        /// Unverified JWT claim.
        jwt: SecretString,
        /// NKey seed used to sign the server challenge.
        nkey_seed: SecretString,
    },
    /// Authenticate with an NKey seed.
    NKey(SecretString),
    /// Authenticate by loading a credentials file.
    CredentialsFile(PathBuf),
}

impl std::fmt::Debug for JetStreamCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => formatter
                .debug_struct("JetStreamCredentials")
                .field("kind", &"none")
                .finish(),
            Self::Token(_) | Self::Jwt { .. } | Self::NKey(_) => formatter
                .debug_struct("JetStreamCredentials")
                .field("kind", &"redacted")
                .finish(),
            Self::CredentialsFile(path) => formatter
                .debug_struct("JetStreamCredentials")
                .field("kind", &"credentials_file")
                .field("path", &path)
                .finish(),
        }
    }
}

/// Validated JetStream publisher configuration.
pub struct JetStreamPublisherConfig {
    enabled: bool,
    nats_url: String,
    credentials: JetStreamCredentials,
    stream_name: String,
    subject: String,
    publish_timeout: Duration,
    max_payload_bytes: usize,
    tls_policy: JetStreamTlsPolicy,
}

/// Input parts used to construct a validated publisher config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JetStreamPublisherConfigInput<'a> {
    /// Whether the publisher is enabled.
    pub enabled: bool,
    /// NATS connection URL.
    pub nats_url: &'a str,
    /// Target stream name.
    pub stream_name: &'a str,
    /// Publish subject.
    pub subject: &'a str,
    /// Request and acknowledgment timeout.
    pub publish_timeout: Duration,
    /// Maximum accepted payload bytes.
    pub max_payload_bytes: usize,
    /// Required TLS policy for the NATS URL.
    pub tls_policy: JetStreamTlsPolicy,
}

impl std::fmt::Debug for JetStreamPublisherConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JetStreamPublisherConfig")
            .field("enabled", &self.enabled)
            .field("nats_url", &redact_nats_url(self.nats_url.as_str()))
            .field("stream_name", &self.stream_name)
            .field("subject", &self.subject)
            .field("publish_timeout", &self.publish_timeout)
            .field("max_payload_bytes", &self.max_payload_bytes)
            .field("tls_policy", &self.tls_policy)
            .finish()
    }
}

impl JetStreamPublisherConfig {
    /// Constructs validated publisher configuration.
    pub fn new(
        enabled: bool,
        nats_url: &str,
        stream_name: &str,
        subject: &str,
        publish_timeout: Duration,
        max_payload_bytes: usize,
    ) -> Result<Self, JetStreamError> {
        Self::new_with_credentials(
            JetStreamPublisherConfigInput {
                enabled,
                nats_url,
                stream_name,
                subject,
                publish_timeout,
                max_payload_bytes,
                tls_policy: JetStreamTlsPolicy::derive_from_url(nats_url)?,
            },
            JetStreamCredentials::None,
        )
    }

    /// Constructs validated publisher configuration with explicit TLS policy and credentials.
    pub fn new_with_credentials(
        input: JetStreamPublisherConfigInput<'_>,
        credentials: JetStreamCredentials,
    ) -> Result<Self, JetStreamError> {
        Self::new_with_tls_policy(input, credentials)
    }

    /// Constructs validated publisher configuration with explicit TLS policy.
    pub fn new_with_tls_policy(
        input: JetStreamPublisherConfigInput<'_>,
        credentials: JetStreamCredentials,
    ) -> Result<Self, JetStreamError> {
        let config = Self {
            enabled: input.enabled,
            nats_url: validate_nats_url(input.nats_url, input.enabled, input.tls_policy)?,
            stream_name: validate_component(
                input.stream_name,
                MAX_STREAM_NAME_BYTES,
                input.enabled,
            )?,
            subject: validate_publish_subject(input.subject, MAX_SUBJECT_BYTES, input.enabled)?,
            credentials,
            publish_timeout: input.publish_timeout,
            max_payload_bytes: input.max_payload_bytes,
            tls_policy: input.tls_policy,
        };

        config.validate()?;
        Ok(config)
    }

    /// Returns whether the publisher is enabled.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the NATS server URL.
    pub fn nats_url(&self) -> &str {
        self.nats_url.as_str()
    }

    /// Returns the configured target stream name.
    pub fn stream_name(&self) -> &str {
        self.stream_name.as_str()
    }

    /// Returns the configured publish subject.
    pub fn subject(&self) -> &str {
        self.subject.as_str()
    }

    /// Returns the configured credentials.
    pub const fn credentials(&self) -> &JetStreamCredentials {
        &self.credentials
    }

    /// Returns this config with explicit credentials.
    pub fn with_credentials(mut self, credentials: JetStreamCredentials) -> Self {
        self.credentials = credentials;
        self
    }

    /// Returns the publish request and ack timeout.
    pub const fn publish_timeout(&self) -> Duration {
        self.publish_timeout
    }

    /// Returns the maximum allowed payload size.
    pub const fn max_payload_bytes(&self) -> usize {
        self.max_payload_bytes
    }

    /// Returns the configured TLS policy.
    pub const fn tls_policy(&self) -> JetStreamTlsPolicy {
        self.tls_policy
    }

    fn validate(&self) -> Result<(), JetStreamError> {
        if !self.enabled {
            return Ok(());
        }

        if self.publish_timeout.is_zero() || self.max_payload_bytes == 0 {
            return Err(JetStreamError::InvalidConfiguration);
        }

        Ok(())
    }
}

/// Validated JetStream pull-consumer configuration.
pub struct JetStreamConsumerConfig {
    enabled: bool,
    nats_url: String,
    stream_name: String,
    consumer_name: String,
    subject: String,
    operation_timeout: Duration,
    ack_timeout: Duration,
    max_deliver: i64,
    deliver_policy: DeliverPolicy,
    replay_policy: ReplayPolicy,
    inactive_threshold: Duration,
    num_replicas: usize,
    max_ack_pending: i64,
    credentials: JetStreamCredentials,
    tls_policy: JetStreamTlsPolicy,
}

/// Input parts used to construct a validated pull-consumer config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JetStreamConsumerConfigInput<'a> {
    /// Whether the consumer is enabled.
    pub enabled: bool,
    /// NATS connection URL.
    pub nats_url: &'a str,
    /// JetStream stream name.
    pub stream_name: &'a str,
    /// Durable consumer name.
    pub consumer_name: &'a str,
    /// Subject filter.
    pub subject: &'a str,
    /// JetStream metadata and pull-operation timeout.
    pub operation_timeout: Duration,
    /// Message acknowledgment timeout.
    pub ack_timeout: Duration,
    /// Max ack pending for the durable consumer.
    /// -1 means unlimited; positive values are valid and capped by the server.
    pub max_ack_pending: i64,
    /// Maximum delivery attempts for a message before parking the message.
    pub max_deliver: i64,
    /// Whether to start delivery from all messages or a later cursor.
    pub deliver_policy: DeliverPolicy,
    /// Whether replay uses rate or instant mode.
    pub replay_policy: ReplayPolicy,
    /// Consumer inactivity threshold.
    pub inactive_threshold: Duration,
    /// Number of consumer replicas.
    pub num_replicas: usize,
    /// Required TLS policy for the NATS URL.
    pub tls_policy: JetStreamTlsPolicy,
}

impl std::fmt::Debug for JetStreamConsumerConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JetStreamConsumerConfig")
            .field("enabled", &self.enabled)
            .field("nats_url", &redact_nats_url(self.nats_url.as_str()))
            .field("stream_name", &self.stream_name)
            .field("consumer_name", &self.consumer_name)
            .field("subject", &self.subject)
            .field("operation_timeout", &self.operation_timeout)
            .field("ack_timeout", &self.ack_timeout)
            .field("max_deliver", &self.max_deliver)
            .field("deliver_policy", &self.deliver_policy)
            .field("replay_policy", &self.replay_policy)
            .field("inactive_threshold", &self.inactive_threshold)
            .field("num_replicas", &self.num_replicas)
            .field("max_ack_pending", &self.max_ack_pending)
            .field("tls_policy", &self.tls_policy)
            .finish()
    }
}

impl JetStreamConsumerConfig {
    /// Constructs validated pull-consumer configuration with explicit credentials.
    pub fn new_with_credentials(
        input: JetStreamConsumerConfigInput<'_>,
        credentials: JetStreamCredentials,
    ) -> Result<Self, JetStreamError> {
        Self::new(input).map(|config| config.with_credentials(credentials))
    }

    /// Constructs validated pull-consumer configuration.
    pub fn new(input: JetStreamConsumerConfigInput<'_>) -> Result<Self, JetStreamError> {
        let config = Self {
            enabled: input.enabled,
            nats_url: validate_nats_url(input.nats_url, input.enabled, input.tls_policy)?,
            stream_name: validate_component(
                input.stream_name,
                MAX_STREAM_NAME_BYTES,
                input.enabled,
            )?,
            consumer_name: validate_component(
                input.consumer_name,
                MAX_CONSUMER_NAME_BYTES,
                input.enabled,
            )?,
            subject: validate_filter_subject(input.subject, MAX_SUBJECT_BYTES, input.enabled)?,
            operation_timeout: input.operation_timeout,
            ack_timeout: input.ack_timeout,
            max_deliver: input.max_deliver,
            deliver_policy: input.deliver_policy,
            replay_policy: input.replay_policy,
            inactive_threshold: input.inactive_threshold,
            num_replicas: input.num_replicas,
            max_ack_pending: input.max_ack_pending,
            credentials: JetStreamCredentials::None,
            tls_policy: input.tls_policy,
        };

        config.validate()?;
        Ok(config)
    }

    /// Returns whether the consumer is enabled.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the NATS server URL.
    pub fn nats_url(&self) -> &str {
        self.nats_url.as_str()
    }

    /// Returns the configured source stream name.
    pub fn stream_name(&self) -> &str {
        self.stream_name.as_str()
    }

    /// Returns the durable consumer name.
    pub fn consumer_name(&self) -> &str {
        self.consumer_name.as_str()
    }

    /// Returns the filter subject.
    pub fn subject(&self) -> &str {
        self.subject.as_str()
    }

    /// Returns the metadata and pull-operation timeout.
    pub const fn operation_timeout(&self) -> Duration {
        self.operation_timeout
    }

    /// Returns the message acknowledgment timeout.
    pub const fn ack_timeout(&self) -> Duration {
        self.ack_timeout
    }

    /// Returns the configured maximum number of message redeliveries.
    pub const fn max_deliver(&self) -> i64 {
        self.max_deliver
    }

    /// Returns the configured consumer delivery policy.
    pub const fn deliver_policy(&self) -> DeliverPolicy {
        self.deliver_policy
    }

    /// Returns the configured consumer replay policy.
    pub const fn replay_policy(&self) -> ReplayPolicy {
        self.replay_policy
    }

    /// Returns the configured consumer inactivity threshold.
    pub const fn inactive_threshold(&self) -> Duration {
        self.inactive_threshold
    }

    /// Returns the configured number of consumer replicas.
    pub const fn num_replicas(&self) -> usize {
        self.num_replicas
    }

    /// Returns the configured credentials.
    pub const fn credentials(&self) -> &JetStreamCredentials {
        &self.credentials
    }

    /// Returns this config with explicit credentials.
    pub fn with_credentials(mut self, credentials: JetStreamCredentials) -> Self {
        self.credentials = credentials;
        self
    }

    /// Returns the max-ack-pending setting.
    pub const fn max_ack_pending(&self) -> i64 {
        self.max_ack_pending
    }

    /// Returns the configured TLS policy.
    pub const fn tls_policy(&self) -> JetStreamTlsPolicy {
        self.tls_policy
    }

    fn validate(&self) -> Result<(), JetStreamError> {
        if !self.enabled {
            return Ok(());
        }

        if self.operation_timeout.is_zero()
            || self.ack_timeout.is_zero()
            || self.inactive_threshold.is_zero()
            || self.max_deliver <= 0
            || self.max_ack_pending == 0
            || self.max_ack_pending < -1
            || self.num_replicas == 0
        {
            return Err(JetStreamError::InvalidConfiguration);
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "config/tests.rs"]
mod tests;
