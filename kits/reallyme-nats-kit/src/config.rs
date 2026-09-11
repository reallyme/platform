// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::IpAddr;
use std::path::PathBuf;
use std::str::{self, FromStr};
use std::sync::Arc;
use std::time::Duration;

use async_nats::ConnectOptions;
use async_nats::jetstream::consumer::{DeliverPolicy, ReplayPolicy};
use async_nats::jetstream::context::ContextBuilder;
use nkeys::KeyPair;
use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::error::JetStreamError;

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

pub(crate) fn validate_nats_url(
    value: &str,
    required: bool,
    tls_policy: JetStreamTlsPolicy,
) -> Result<String, JetStreamError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return if required {
            Err(JetStreamError::InvalidConfiguration)
        } else {
            Ok(String::new())
        };
    }

    if trimmed.len() > MAX_NATS_URL_BYTES || trimmed.chars().any(char::is_whitespace) {
        return Err(JetStreamError::InvalidConfiguration);
    }

    let url = Url::parse(trimmed).map_err(|_| JetStreamError::InvalidConfiguration)?;

    if url.username() != "" || url.password().is_some() {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if url.scheme().is_empty() {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if matches!(
        tls_policy,
        JetStreamTlsPolicy::Required if !url.scheme().eq_ignore_ascii_case("tls")
    ) {
        return Err(JetStreamError::InvalidConfiguration);
    }

    let scheme = url.scheme().to_ascii_lowercase();
    if !matches!(scheme.as_str(), "nats" | "tls") {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if url.host().is_none() {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if !url.path().is_empty() && url.path() != "/" {
        return Err(JetStreamError::InvalidConfiguration);
    }

    if url.query().is_some() || url.fragment().is_some() {
        return Err(JetStreamError::InvalidConfiguration);
    }

    Ok(trimmed.to_owned())
}

fn validate_publish_subject(
    value: &str,
    max_bytes: usize,
    required: bool,
) -> Result<String, JetStreamError> {
    let subject = validate_component(value, max_bytes, required)?;
    if subject.contains('*') || subject.contains('>') {
        return Err(JetStreamError::InvalidConfiguration);
    }

    Ok(subject)
}

fn validate_filter_subject(
    value: &str,
    max_bytes: usize,
    required: bool,
) -> Result<String, JetStreamError> {
    validate_component(value, max_bytes, required)
}

fn validate_component(
    value: &str,
    max_bytes: usize,
    required: bool,
) -> Result<String, JetStreamError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return if required {
            Err(JetStreamError::InvalidConfiguration)
        } else {
            Ok(String::new())
        };
    }

    if trimmed.len() > max_bytes || trimmed.chars().any(char::is_whitespace) {
        return Err(JetStreamError::InvalidConfiguration);
    }

    Ok(trimmed.to_owned())
}

impl JetStreamTlsPolicy {
    /// Derives the default TLS policy for a URL from host locality and scheme.
    pub fn derive_from_url(nats_url: &str) -> Result<Self, JetStreamError> {
        if nats_url.is_empty() {
            return Ok(Self::Optional);
        }

        let trimmed = nats_url.trim();
        let host = parse_nats_host(trimmed).ok_or(JetStreamError::InvalidConfiguration)?;
        let scheme = parse_nats_scheme(trimmed).ok_or(JetStreamError::InvalidConfiguration)?;
        if scheme.eq_ignore_ascii_case("tls") {
            return Ok(Self::Required);
        }
        if is_private_network_host(&host) {
            Ok(Self::Disabled)
        } else {
            Ok(Self::Required)
        }
    }
}

fn parse_nats_scheme(value: &str) -> Option<String> {
    Url::parse(value).ok().and_then(|parsed| {
        if parsed.scheme().is_empty() {
            return None;
        }

        Some(parsed.scheme().to_owned())
    })
}

fn parse_nats_host(value: &str) -> Option<String> {
    Url::parse(value)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
}

pub(crate) fn redact_nats_url(value: &str) -> String {
    let Ok(url) = Url::parse(value) else {
        return String::from("<invalid-url>");
    };

    let mut redacted = String::new();
    redacted.push_str(url.scheme());
    redacted.push_str("://");

    if let Some(host) = url.host_str() {
        redacted.push_str(host);
    }

    if let Some(port) = url.port() {
        redacted.push(':');
        redacted.push_str(&port.to_string());
    }

    if url.host().is_none() {
        return redacted;
    }

    if url.path() != "/" && !url.path().is_empty() {
        redacted.push_str(url.path());
    }

    redacted
}

fn is_private_network_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let host = host.to_ascii_lowercase();
    if host.ends_with(LOCAL_SUBJECT_SUFFIX) {
        return true;
    }

    if let Ok(ip) = IpAddr::from_str(&host) {
        return match ip {
            IpAddr::V4(ipv4) => ipv4.is_loopback() || ipv4.is_private() || ipv4.is_unspecified(),
            IpAddr::V6(ipv6) => ipv6.is_loopback() || ipv6.is_unspecified(),
        };
    }

    !host.contains('.')
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_nats::jetstream::consumer::{DeliverPolicy, ReplayPolicy};
    use secrecy::{ExposeSecret, SecretString};

    use super::{
        JetStreamConsumerConfig, JetStreamConsumerConfigInput, JetStreamCredentials,
        JetStreamError, JetStreamPublisherConfig, JetStreamPublisherConfigInput,
        JetStreamTlsPolicy,
    };

    #[test]
    fn publisher_config_rejects_missing_stream_when_enabled() {
        let result = JetStreamPublisherConfig::new(
            true,
            "nats://127.0.0.1:4222",
            "",
            "search.spider",
            Duration::from_secs(5),
            1024,
        );

        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[test]
    fn publisher_config_allows_empty_fields_when_disabled() {
        let result = JetStreamPublisherConfig::new(false, "", "", "", Duration::from_secs(1), 1);

        assert!(result.is_ok());
    }

    #[test]
    fn publisher_config_rejects_http_scheme() {
        let result = JetStreamPublisherConfig::new(
            true,
            "http://127.0.0.1:4222",
            "updates",
            "search.spider",
            Duration::from_secs(5),
            1024,
        );

        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[test]
    fn publisher_config_rejects_required_tls_policy_with_nats_scheme() {
        let result = JetStreamPublisherConfig::new_with_tls_policy(
            JetStreamPublisherConfigInput {
                enabled: true,
                nats_url: "nats://127.0.0.1:4222",
                stream_name: "search_spider",
                subject: "search.requests",
                publish_timeout: Duration::from_secs(5),
                max_payload_bytes: 1024,
                tls_policy: JetStreamTlsPolicy::Required,
            },
            JetStreamCredentials::None,
        );

        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[test]
    fn publisher_config_accepts_required_tls_policy_with_tls_scheme() {
        let result = JetStreamPublisherConfig::new_with_tls_policy(
            JetStreamPublisherConfigInput {
                enabled: true,
                nats_url: "tls://127.0.0.1:4222",
                stream_name: "search_spider",
                subject: "search.requests",
                publish_timeout: Duration::from_secs(5),
                max_payload_bytes: 1024,
                tls_policy: JetStreamTlsPolicy::Required,
            },
            JetStreamCredentials::None,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn publisher_config_rejects_url_with_userinfo() {
        let result = JetStreamPublisherConfig::new(
            true,
            "nats://token@127.0.0.1:4222",
            "updates",
            "search.spider",
            Duration::from_secs(5),
            1024,
        );

        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[test]
    fn consumer_config_rejects_zero_ack_pending() {
        let result = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
            enabled: true,
            nats_url: "nats://127.0.0.1:4222",
            stream_name: "updates",
            consumer_name: "worker",
            subject: "updates.local",
            operation_timeout: Duration::from_secs(5),
            ack_timeout: Duration::from_secs(5),
            max_ack_pending: 0,
            max_deliver: 1,
            deliver_policy: DeliverPolicy::All,
            replay_policy: ReplayPolicy::Instant,
            inactive_threshold: Duration::from_secs(30),
            num_replicas: 1,
            tls_policy: JetStreamTlsPolicy::Disabled,
        });

        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[test]
    fn consumer_config_rejects_negative_unlimited_overflow() {
        let result = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
            enabled: true,
            nats_url: "nats://127.0.0.1:4222",
            stream_name: "updates",
            consumer_name: "worker",
            subject: "updates.local",
            operation_timeout: Duration::from_secs(5),
            ack_timeout: Duration::from_secs(5),
            max_ack_pending: -2,
            max_deliver: 1,
            deliver_policy: DeliverPolicy::All,
            replay_policy: ReplayPolicy::Instant,
            inactive_threshold: Duration::from_secs(30),
            num_replicas: 1,
            tls_policy: JetStreamTlsPolicy::Disabled,
        });

        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[test]
    fn consumer_config_rejects_zero_ack_timeout() {
        let result = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
            enabled: true,
            nats_url: "nats://127.0.0.1:4222",
            stream_name: "updates",
            consumer_name: "worker",
            subject: "updates.local",
            operation_timeout: Duration::from_secs(5),
            ack_timeout: Duration::from_secs(0),
            max_ack_pending: 1,
            max_deliver: 1,
            deliver_policy: DeliverPolicy::All,
            replay_policy: ReplayPolicy::Instant,
            inactive_threshold: Duration::from_secs(30),
            num_replicas: 1,
            tls_policy: JetStreamTlsPolicy::Disabled,
        });

        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[test]
    fn consumer_config_accepts_unlimited_max_ack_pending() {
        let result = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
            enabled: true,
            nats_url: "nats://127.0.0.1:4222",
            stream_name: "updates",
            consumer_name: "worker",
            subject: "updates.local",
            operation_timeout: Duration::from_secs(5),
            ack_timeout: Duration::from_secs(5),
            max_ack_pending: -1,
            max_deliver: 1,
            deliver_policy: DeliverPolicy::All,
            replay_policy: ReplayPolicy::Instant,
            inactive_threshold: Duration::from_secs(30),
            num_replicas: 1,
            tls_policy: JetStreamTlsPolicy::Disabled,
        });

        assert!(result.is_ok());
    }

    #[test]
    fn consumer_config_rejects_zero_max_deliveries() {
        let result = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
            enabled: true,
            nats_url: "nats://127.0.0.1:4222",
            stream_name: "updates",
            consumer_name: "worker",
            subject: "updates.local",
            operation_timeout: Duration::from_secs(5),
            ack_timeout: Duration::from_secs(5),
            max_ack_pending: 1,
            max_deliver: 0,
            deliver_policy: DeliverPolicy::All,
            replay_policy: ReplayPolicy::Instant,
            inactive_threshold: Duration::from_secs(30),
            num_replicas: 1,
            tls_policy: JetStreamTlsPolicy::Disabled,
        });

        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[test]
    fn validate_nats_url_rejects_http_scheme() {
        let result = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
            enabled: true,
            nats_url: "http://127.0.0.1:4222",
            stream_name: "updates",
            consumer_name: "worker",
            subject: "updates.local",
            operation_timeout: Duration::from_secs(5),
            ack_timeout: Duration::from_secs(5),
            max_ack_pending: -1,
            max_deliver: 1,
            deliver_policy: DeliverPolicy::All,
            replay_policy: ReplayPolicy::Instant,
            inactive_threshold: Duration::from_secs(30),
            num_replicas: 1,
            tls_policy: JetStreamTlsPolicy::Disabled,
        });

        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[test]
    fn validate_subject_rejects_publish_wildcards() {
        let result = JetStreamPublisherConfig::new(
            true,
            "nats://127.0.0.1:4222",
            "updates",
            "updates.*",
            Duration::from_secs(5),
            1024,
        );
        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[test]
    fn validate_filter_subject_allows_wildcards() {
        let result = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
            enabled: true,
            nats_url: "nats://127.0.0.1:4222",
            stream_name: "updates",
            consumer_name: "worker",
            subject: "updates.*",
            operation_timeout: Duration::from_secs(5),
            ack_timeout: Duration::from_secs(5),
            max_ack_pending: 1,
            max_deliver: 1,
            deliver_policy: DeliverPolicy::All,
            replay_policy: ReplayPolicy::Instant,
            inactive_threshold: Duration::from_secs(30),
            num_replicas: 1,
            tls_policy: JetStreamTlsPolicy::Disabled,
        });

        assert!(result.is_ok());
    }

    #[test]
    fn derive_tls_policy_defaults_for_local_host() {
        let policy = JetStreamTlsPolicy::derive_from_url("nats://127.0.0.1:4222");
        assert_eq!(policy, Ok(JetStreamTlsPolicy::Disabled));
    }

    #[test]
    fn derive_tls_policy_requires_tls_for_public_host() {
        let policy = JetStreamTlsPolicy::derive_from_url("nats://198.51.100.42:4222");
        assert_eq!(policy, Ok(JetStreamTlsPolicy::Required));
    }

    #[test]
    fn derive_tls_policy_respects_tls_scheme() {
        let policy = JetStreamTlsPolicy::derive_from_url("TLS://198.51.100.42:4222");
        assert_eq!(policy, Ok(JetStreamTlsPolicy::Required));
    }

    #[test]
    fn publisher_config_new_with_credentials_keeps_url_clean() {
        let result = JetStreamPublisherConfig::new_with_credentials(
            JetStreamPublisherConfigInput {
                enabled: true,
                nats_url: "nats://127.0.0.1:4222",
                stream_name: "search_spider",
                subject: "search.requests",
                publish_timeout: Duration::from_secs(5),
                max_payload_bytes: 64,
                tls_policy: JetStreamTlsPolicy::Disabled,
            },
            JetStreamCredentials::Token(SecretString::new("token-from-secrets".to_string().into())),
        );
        assert!(result.is_ok());
        let Ok(config) = result else {
            unreachable!();
        };
        assert!(format!("{:?}", config.credentials()).contains("redacted"));
        let debug = format!("{config:?}");
        assert!(!debug.contains("token-from-secrets"));
    }

    #[test]
    fn consumer_config_new_with_credentials_keeps_url_clean() {
        let result = JetStreamConsumerConfig::new_with_credentials(
            JetStreamConsumerConfigInput {
                enabled: true,
                nats_url: "nats://127.0.0.1:4222",
                stream_name: "updates",
                consumer_name: "worker",
                subject: "updates.local",
                operation_timeout: Duration::from_secs(5),
                ack_timeout: Duration::from_secs(5),
                max_ack_pending: 1,
                max_deliver: 1,
                deliver_policy: DeliverPolicy::All,
                replay_policy: ReplayPolicy::Instant,
                inactive_threshold: Duration::from_secs(30),
                num_replicas: 1,
                tls_policy: JetStreamTlsPolicy::Disabled,
            },
            JetStreamCredentials::NKey(SecretString::new("SAMPLESEED".to_string().into())),
        );
        assert!(result.is_ok());
        let Ok(config) = result else {
            unreachable!();
        };
        assert!(matches!(
            config.credentials(),
            JetStreamCredentials::NKey(seed) if seed.expose_secret() == "SAMPLESEED"
        ));
        let debug = format!("{config:?}");
        assert!(!debug.contains("SAMPLESEED"));
    }

    #[test]
    fn config_debug_redacts_url_userinfo() {
        let result = JetStreamPublisherConfig::new(
            true,
            "nats://127.0.0.1:4222",
            "search_spider",
            "search.requests",
            Duration::from_secs(5),
            16,
        );
        assert!(result.is_ok(), "valid config");
        let Ok(config) = result else {
            unreachable!();
        };

        let debug = format!("{config:?}");
        assert!(!debug.contains("token"));
        assert!(!debug.contains("username"));
        assert!(!debug.contains("password"));
    }
}
