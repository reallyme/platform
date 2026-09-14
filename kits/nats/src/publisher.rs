// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use metrics::{counter, histogram};

use crate::{
    config::{JetStreamPublisherConfig, connect_with_credentials, create_context},
    error::JetStreamError,
    message_id::deterministic_message_id_for_payload,
};

const METRIC_NATS_PUBLISHER_PUBLISH_TOTAL: &str = "reallyme_nats_kit_publisher_publish_total";
const METRIC_NATS_PUBLISHER_PUBLISH_FAILURES_TOTAL: &str =
    "reallyme_nats_kit_publisher_publish_failures_total";
const METRIC_NATS_PUBLISHER_PUBLISH_DURATION_SECONDS: &str =
    "reallyme_nats_kit_publisher_publish_duration_seconds";
const METRIC_NATS_PUBLISHER_VALIDATE_TOTAL: &str = "reallyme_nats_kit_publisher_validate_total";
const METRIC_NATS_PUBLISHER_CONNECT_TOTAL: &str = "reallyme_nats_kit_publisher_connect_total";
const METRIC_NATS_PUBLISHER_CONNECT_DURATION_SECONDS: &str =
    "reallyme_nats_kit_publisher_connect_duration_seconds";
const METRIC_NATS_PUBLISHER_VALIDATE_FAILURES_TOTAL: &str =
    "reallyme_nats_kit_publisher_validate_failures_total";
const METRIC_NATS_PUBLISHER_VALIDATE_DURATION_SECONDS: &str =
    "reallyme_nats_kit_publisher_validate_duration_seconds";
const METRIC_NATS_PUBLISHER_ACK_TOTAL: &str = "reallyme_nats_kit_publisher_ack_total";
const METRIC_NATS_PUBLISHER_ACK_DURATION_SECONDS: &str =
    "reallyme_nats_kit_publisher_ack_duration_seconds";

/// Stable JetStream publish acknowledgment details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JetStreamPublishAck {
    stream_name: String,
    sequence: u64,
    duplicate: bool,
}

impl JetStreamPublishAck {
    /// Constructs a publish acknowledgment.
    pub fn new(stream_name: &str, sequence: u64, duplicate: bool) -> Result<Self, JetStreamError> {
        if stream_name.is_empty() {
            return Err(JetStreamError::PublishNotAcknowledged);
        }

        Ok(Self {
            stream_name: stream_name.to_owned(),
            sequence,
            duplicate,
        })
    }

    /// Returns the acknowledged stream name.
    pub fn stream_name(&self) -> &str {
        self.stream_name.as_str()
    }

    /// Returns the acknowledged stream sequence.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns whether JetStream marked the publish as a duplicate.
    pub const fn duplicate(&self) -> bool {
        self.duplicate
    }
}

/// Backend abstraction used by the shared JetStream publisher.
#[allow(async_fn_in_trait)]
pub trait JetStreamPublisherBackend: Send + Sync {
    /// Validates that the backend can reach the configured stream.
    async fn validate_startup(
        &self,
        config: &JetStreamPublisherConfig,
    ) -> Result<(), JetStreamError>;

    /// Publishes bytes and waits for a JetStream publish acknowledgment.
    async fn publish(
        &self,
        config: &JetStreamPublisherConfig,
        payload: Bytes,
        message_id: Option<String>,
    ) -> Result<JetStreamPublishAck, JetStreamError>;
}

impl<T> JetStreamPublisherBackend for std::sync::Arc<T>
where
    T: JetStreamPublisherBackend,
{
    async fn validate_startup(
        &self,
        config: &JetStreamPublisherConfig,
    ) -> Result<(), JetStreamError> {
        (**self).validate_startup(config).await
    }

    async fn publish(
        &self,
        config: &JetStreamPublisherConfig,
        payload: Bytes,
        message_id: Option<String>,
    ) -> Result<JetStreamPublishAck, JetStreamError> {
        (**self).publish(config, payload, message_id).await
    }
}

/// Shared JetStream publisher with ack handling and payload limits.
#[derive(Clone)]
pub struct JetStreamPublisher<B = ContextPublisherBackend>
where
    B: JetStreamPublisherBackend,
{
    config: Arc<JetStreamPublisherConfig>,
    backend: B,
}

impl JetStreamPublisher<ContextPublisherBackend> {
    /// Connects a publisher-backed JetStream context using the provided config.
    pub async fn connect(config: JetStreamPublisherConfig) -> Result<Self, JetStreamError> {
        if !config.enabled() {
            return Err(JetStreamError::Disabled);
        }

        let started = std::time::Instant::now();
        tracing::info!(
            stream = config.stream_name(),
            "connecting JetStream publisher"
        );
        let result = async {
            connect_with_credentials(config.nats_url(), config.tls_policy(), config.credentials())
                .await
        }
        .await;
        if let Err(error) = &result {
            tracing::warn!(
                stream = config.stream_name(),
                error = ?error,
                "publisher connect failed"
            );
        }
        let status = if result.is_ok() { "ok" } else { "error" };
        let _ = counter!(METRIC_NATS_PUBLISHER_CONNECT_TOTAL, "result" => status);
        histogram!(
            METRIC_NATS_PUBLISHER_CONNECT_DURATION_SECONDS,
            "result" => status
        )
        .record(started.elapsed().as_secs_f64());
        let client = result?;
        Ok(Self::from_client(client, config))
    }

    /// Constructs a publisher from an existing NATS client and validated config.
    ///
    /// `publish_timeout` is used for both the client request timeout and the public publish
    /// operation timeout, so stalled publish requests are bounded by one explicit budget.
    pub fn from_client(client: async_nats::Client, config: JetStreamPublisherConfig) -> Self {
        let context = create_context(client, config.publish_timeout(), config.publish_timeout());
        Self {
            config: Arc::new(config),
            backend: ContextPublisherBackend::new(context),
        }
    }
}

impl<B> JetStreamPublisher<B>
where
    B: JetStreamPublisherBackend,
{
    /// Constructs a publisher from a validated config and custom backend.
    pub fn new_with_backend(config: JetStreamPublisherConfig, backend: B) -> Self {
        Self {
            config: Arc::new(config),
            backend,
        }
    }

    /// Returns the validated config.
    pub fn config(&self) -> &JetStreamPublisherConfig {
        self.config.as_ref()
    }

    /// Validates startup access to the configured stream.
    pub async fn validate_startup(&self) -> Result<(), JetStreamError> {
        if !self.config.enabled() {
            return Err(JetStreamError::Disabled);
        }

        let started = Instant::now();
        tracing::info!(
            stream = self.config.stream_name(),
            "validating JetStream publisher startup"
        );
        let result = self.backend.validate_startup(&self.config).await;
        if let Err(error) = &result {
            tracing::warn!(
                stream = self.config.stream_name(),
                error = ?error,
                "publisher startup validation failed"
            );
            let _ = counter!(
                METRIC_NATS_PUBLISHER_VALIDATE_FAILURES_TOTAL,
                "reason" => "backend_error"
            );
        }

        let status = if result.is_ok() { "ok" } else { "error" };
        let _ = counter!(METRIC_NATS_PUBLISHER_VALIDATE_TOTAL, "result" => status);
        histogram!(
            METRIC_NATS_PUBLISHER_VALIDATE_DURATION_SECONDS,
            "result" => status
        )
        .record(started.elapsed().as_secs_f64());

        result
    }

    /// Publishes a protobuf or other binary payload and waits for publish ack.
    pub async fn publish_bytes(
        &self,
        payload: impl Into<Bytes>,
        message_id: Option<&str>,
    ) -> Result<JetStreamPublishAck, JetStreamError> {
        let payload = payload.into();
        let started = Instant::now();

        if !self.config.enabled() {
            return Err(JetStreamError::Disabled);
        }

        if payload.len() > self.config.max_payload_bytes() {
            return Err(JetStreamError::PayloadTooLarge);
        }

        let message_id_value = message_id.map(str::to_owned);
        let message_id = if message_id_value.is_some() {
            "provided"
        } else {
            "absent"
        };
        tracing::info!(
            stream = self.config.stream_name(),
            message_id = message_id,
            "publishing JetStream payload"
        );
        let result = self
            .backend
            .publish(&self.config, payload, message_id_value)
            .await;
        if let Err(error) = &result {
            tracing::warn!(
                stream = self.config.stream_name(),
                message_id = message_id,
                error = ?error,
                "publisher publish_bytes failed"
            );
        }
        let status = if result.is_ok() { "ok" } else { "error" };
        let _ = counter!(
            METRIC_NATS_PUBLISHER_PUBLISH_TOTAL,
            "result" => status,
            "message_id" => message_id
        );
        histogram!(
            METRIC_NATS_PUBLISHER_PUBLISH_DURATION_SECONDS,
            "result" => status,
            "message_id" => message_id
        )
        .record(started.elapsed().as_secs_f64());
        if result.is_err() {
            let _ = counter!(METRIC_NATS_PUBLISHER_PUBLISH_FAILURES_TOTAL, "result" => status);
        }

        result
    }

    /// Publishes bytes using a deterministic message id derived from the
    /// configured subject and payload, enabling JetStream publish dedupe.
    pub async fn publish_bytes_deduplicated(
        &self,
        payload: impl Into<Bytes>,
    ) -> Result<JetStreamPublishAck, JetStreamError> {
        let payload = payload.into();
        let message_id =
            deterministic_message_id_for_payload(self.config.subject(), payload.as_ref());

        self.publish_bytes(payload, Some(message_id.as_str())).await
    }
}

impl<B> std::fmt::Debug for JetStreamPublisher<B>
where
    B: JetStreamPublisherBackend,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JetStreamPublisher")
            .field("enabled", &self.config.enabled())
            .field("stream_name", &self.config.stream_name())
            .field("subject", &self.config.subject())
            .field(
                "publish_timeout_millis",
                &self.config.publish_timeout().as_millis(),
            )
            .field("max_payload_bytes", &self.config.max_payload_bytes())
            .finish()
    }
}

#[derive(Clone)]
/// Internal JetStream context-backed publisher with stream max-size caching.
pub struct ContextPublisherBackend {
    context: async_nats::jetstream::Context,
    max_message_sizes: Arc<tokio::sync::RwLock<std::collections::HashMap<String, Option<usize>>>>,
}

impl ContextPublisherBackend {
    fn new(context: async_nats::jetstream::Context) -> Self {
        Self {
            context,
            max_message_sizes: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl JetStreamPublisherBackend for ContextPublisherBackend {
    async fn validate_startup(
        &self,
        config: &JetStreamPublisherConfig,
    ) -> Result<(), JetStreamError> {
        let stream = self
            .context
            .get_stream(config.stream_name().to_owned())
            .await
            .map_err(|_| JetStreamError::StreamLookupFailed)?;
        let info = stream
            .get_info()
            .await
            .map_err(|_| JetStreamError::StreamLookupFailed)?;

        let max_message_size = Self::normalize_max_message_size(info.config.max_message_size)?;
        let mut max_message_sizes = self.max_message_sizes.write().await;
        max_message_sizes.insert(config.stream_name().to_owned(), max_message_size);
        Ok(())
    }

    async fn publish(
        &self,
        config: &JetStreamPublisherConfig,
        payload: Bytes,
        message_id: Option<String>,
    ) -> Result<JetStreamPublishAck, JetStreamError> {
        if payload.len() > config.max_payload_bytes() {
            return Err(JetStreamError::PayloadTooLarge);
        }

        if let Some(max_message_size) = self.stream_max_message_size(config.stream_name()).await?
            && payload.len() > max_message_size
        {
            return Err(JetStreamError::PayloadTooLarge);
        }

        let message_id = message_id.as_deref();
        let message_id_label = if message_id.is_some() {
            "provided"
        } else {
            "absent"
        };
        let publish = if let Some(message_id) = message_id {
            async_nats::jetstream::message::PublishMessage::build()
                .payload(payload)
                .message_id(message_id)
                .expected_stream(config.stream_name())
        } else {
            async_nats::jetstream::message::PublishMessage::build()
                .payload(payload)
                .expected_stream(config.stream_name())
        };
        let subject = config.subject().to_owned();

        let started = Instant::now();
        let ack = tokio::time::timeout(
            config.publish_timeout(),
            self.context.send_publish(subject, publish),
        )
        .await
        .map_err(|_| {
            tracing::warn!(
                stream = config.stream_name(),
                error = "publisher send_publish timeout"
            );
            JetStreamError::PublishNotAcknowledged
        })?
        .map_err(|_| {
            tracing::warn!(
                stream = config.stream_name(),
                error = "publisher send_publish failed"
            );
            JetStreamError::PublishNotAcknowledged
        })?;
        let ack = ack.await.map_err(|_| {
            tracing::warn!(
                stream = config.stream_name(),
                error = "publisher send ack timeout"
            );
            JetStreamError::PublishNotAcknowledged
        })?;
        let _ = counter!(
            METRIC_NATS_PUBLISHER_ACK_TOTAL,
            "result" => "ok",
            "message_id" => message_id_label
        );
        histogram!(
            METRIC_NATS_PUBLISHER_ACK_DURATION_SECONDS,
            "result" => "ok",
            "message_id" => message_id_label
        )
        .record(started.elapsed().as_secs_f64());

        Ok(JetStreamPublishAck {
            stream_name: ack.stream.to_owned(),
            sequence: ack.sequence,
            duplicate: ack.duplicate,
        })
    }
}

impl ContextPublisherBackend {
    async fn stream_max_message_size(
        &self,
        stream_name: &str,
    ) -> Result<Option<usize>, JetStreamError> {
        {
            let cached = self.max_message_sizes.read().await;
            if let Some(max_message_size) = cached.get(stream_name) {
                return Ok(*max_message_size);
            }
        }

        let stream = self
            .context
            .get_stream(stream_name.to_owned())
            .await
            .map_err(|_| JetStreamError::StreamLookupFailed)?;
        let info = stream
            .get_info()
            .await
            .map_err(|_| JetStreamError::StreamLookupFailed)?;

        let max_message_size = Self::normalize_max_message_size(info.config.max_message_size)?;
        let mut max_message_sizes = self.max_message_sizes.write().await;
        max_message_sizes.insert(stream_name.to_owned(), max_message_size);

        Ok(max_message_size)
    }

    fn normalize_max_message_size(raw: i32) -> Result<Option<usize>, JetStreamError> {
        if raw <= 0 {
            return Ok(None);
        }

        Ok(Some(
            usize::try_from(raw).map_err(|_| JetStreamError::StreamLookupFailed)?,
        ))
    }
}

#[cfg(test)]
#[path = "publisher_tests.rs"]
mod tests;
