// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::StreamExt;
use futures_util::stream::{self, BoxStream};
use metrics::{counter, histogram};

use crate::{
    config::{JetStreamConsumerConfig, connect_with_credentials, create_context},
    error::JetStreamError,
};

const METRIC_NATS_CONSUMER_PULL_TOTAL: &str = "reallyme_nats_kit_consumer_pull_total";
const METRIC_NATS_CONSUMER_PULL_FAILURES_TOTAL: &str =
    "reallyme_nats_kit_consumer_pull_failures_total";
const METRIC_NATS_CONSUMER_PULL_DURATION_SECONDS: &str =
    "reallyme_nats_kit_consumer_pull_duration_seconds";
const METRIC_NATS_CONSUMER_DELIVERY_TOTAL: &str = "reallyme_nats_kit_consumer_delivery_total";
const METRIC_NATS_CONSUMER_DELIVERY_FAILURE_TOTAL: &str =
    "reallyme_nats_kit_consumer_delivery_failure_total";
const METRIC_NATS_CONSUMER_DELIVERY_ACK_TOTAL: &str =
    "reallyme_nats_kit_consumer_delivery_ack_total";
const METRIC_NATS_CONSUMER_DELIVERY_ACK_DURATION_SECONDS: &str =
    "reallyme_nats_kit_consumer_delivery_ack_duration_seconds";
const METRIC_NATS_CONSUMER_VALIDATE_TOTAL: &str = "reallyme_nats_kit_consumer_validate_total";
const METRIC_NATS_CONSUMER_CONNECT_TOTAL: &str = "reallyme_nats_kit_consumer_connect_total";
const METRIC_NATS_CONSUMER_VALIDATE_FAILURES_TOTAL: &str =
    "reallyme_nats_kit_consumer_validate_failures_total";
const METRIC_NATS_CONSUMER_VALIDATE_DURATION_SECONDS: &str =
    "reallyme_nats_kit_consumer_validate_duration_seconds";
const METRIC_NATS_CONSUMER_CONNECT_DURATION_SECONDS: &str =
    "reallyme_nats_kit_consumer_connect_duration_seconds";

const MAX_PULL_ATTEMPTS: usize = 2;

/// Supported JetStream acknowledgment dispositions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JetStreamAckDisposition {
    /// Positively acknowledge successful handling.
    Ack,
    /// Negative-acknowledge for later retry.
    Nak,
    /// Terminate further redelivery attempts.
    Term,
}

/// Reusable JetStream delivery metadata extracted from the server ack subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JetStreamDeliveryInfo {
    stream_sequence: Option<u64>,
    consumer_sequence: Option<u64>,
    pending: Option<u64>,
}

impl JetStreamDeliveryInfo {
    /// Constructs delivery metadata.
    pub const fn new(
        stream_sequence: Option<u64>,
        consumer_sequence: Option<u64>,
        pending: Option<u64>,
    ) -> Self {
        Self {
            stream_sequence,
            consumer_sequence,
            pending,
        }
    }

    /// Returns the JetStream stream sequence when available.
    pub const fn stream_sequence(&self) -> Option<u64> {
        self.stream_sequence
    }

    /// Returns the JetStream consumer sequence when available.
    pub const fn consumer_sequence(&self) -> Option<u64> {
        self.consumer_sequence
    }

    /// Returns the remaining pending message count when available.
    pub const fn pending(&self) -> Option<u64> {
        self.pending
    }
}

/// Generic consumed JetStream delivery with ack/nak/term helpers.
pub struct JetStreamDelivery {
    subject: String,
    payload: Bytes,
    headers: Option<async_nats::HeaderMap>,
    info: JetStreamDeliveryInfo,
    acker: JetStreamDeliveryAcker,
}

#[allow(clippy::large_enum_variant)]
enum JetStreamDeliveryAcker {
    Context(ContextDeliveryAcker),
    Fake {
        dispositions: std::sync::Arc<std::sync::Mutex<Vec<JetStreamAckDisposition>>>,
    },
}

impl JetStreamDelivery {
    fn new(
        subject: String,
        payload: Bytes,
        headers: Option<async_nats::HeaderMap>,
        info: JetStreamDeliveryInfo,
        message: async_nats::jetstream::Message,
        ack_timeout: Duration,
    ) -> Self {
        Self {
            subject,
            payload,
            headers,
            info,
            acker: JetStreamDeliveryAcker::Context(ContextDeliveryAcker::new(message, ack_timeout)),
        }
    }

    pub(crate) fn new_for_test(
        subject: String,
        payload: Bytes,
        headers: Option<async_nats::HeaderMap>,
        info: JetStreamDeliveryInfo,
        dispositions: std::sync::Arc<std::sync::Mutex<Vec<JetStreamAckDisposition>>>,
    ) -> Self {
        Self {
            subject,
            payload,
            headers,
            info,
            acker: JetStreamDeliveryAcker::Fake { dispositions },
        }
    }

    /// Returns the message subject.
    pub fn subject(&self) -> &str {
        self.subject.as_str()
    }

    /// Returns the message payload bytes.
    pub fn payload(&self) -> &[u8] {
        self.payload.as_ref()
    }

    /// Returns the optional message headers.
    pub fn headers(&self) -> Option<&async_nats::HeaderMap> {
        self.headers.as_ref()
    }

    /// Returns parsed JetStream delivery metadata.
    pub const fn info(&self) -> JetStreamDeliveryInfo {
        self.info
    }

    /// Sends an explicit positive acknowledgment.
    pub async fn ack(&self) -> Result<(), JetStreamError> {
        self.acknowledge(JetStreamAckDisposition::Ack, None).await
    }

    /// Sends a positive acknowledgment and waits for server confirmation.
    pub async fn ack_confirmed(&self) -> Result<(), JetStreamError> {
        self.acknowledge_confirmed().await
    }

    /// Sends a negative acknowledgment without an explicit delay override.
    pub async fn nak(&self) -> Result<(), JetStreamError> {
        self.acknowledge(JetStreamAckDisposition::Nak, None).await
    }

    /// Sends a negative acknowledgment with an explicit redelivery delay.
    pub async fn nak_with_delay(&self, delay: Duration) -> Result<(), JetStreamError> {
        self.acknowledge(JetStreamAckDisposition::Nak, Some(delay))
            .await
    }

    /// Terminates further redelivery attempts.
    pub async fn term(&self) -> Result<(), JetStreamError> {
        self.acknowledge(JetStreamAckDisposition::Term, None).await
    }

    async fn acknowledge(
        &self,
        disposition: JetStreamAckDisposition,
        delay: Option<Duration>,
    ) -> Result<(), JetStreamError> {
        let started = Instant::now();
        let result = match &self.acker {
            JetStreamDeliveryAcker::Context(acker) => acker.acknowledge(disposition, delay).await,
            JetStreamDeliveryAcker::Fake { dispositions } => {
                dispositions
                    .lock()
                    .map_err(|_| JetStreamError::SyncPrimitivePoisoned)?
                    .push(disposition);
                Ok(())
            }
        };

        let disposition_label = match disposition {
            JetStreamAckDisposition::Ack => "ack",
            JetStreamAckDisposition::Nak => "nak",
            JetStreamAckDisposition::Term => "term",
        };
        let status_label = if result.is_ok() { "ok" } else { "error" };
        let _ = counter!(
            METRIC_NATS_CONSUMER_DELIVERY_ACK_TOTAL,
            "disposition" => disposition_label,
            "result" => status_label
        );
        histogram!(
            METRIC_NATS_CONSUMER_DELIVERY_ACK_DURATION_SECONDS,
            "disposition" => disposition_label,
            "result" => status_label
        )
        .record(started.elapsed().as_secs_f64());

        result
    }

    async fn acknowledge_confirmed(&self) -> Result<(), JetStreamError> {
        let started = Instant::now();
        let result = match &self.acker {
            JetStreamDeliveryAcker::Context(acker) => acker.acknowledge_confirmed().await,
            JetStreamDeliveryAcker::Fake { dispositions } => {
                dispositions
                    .lock()
                    .map_err(|_| JetStreamError::SyncPrimitivePoisoned)?
                    .push(JetStreamAckDisposition::Ack);
                Ok(())
            }
        };

        let status = if result.is_ok() { "ok" } else { "error" };
        let _ = counter!(
            METRIC_NATS_CONSUMER_DELIVERY_ACK_TOTAL,
            "disposition" => "ack_confirmed",
            "result" => status
        );
        histogram!(
            METRIC_NATS_CONSUMER_DELIVERY_ACK_DURATION_SECONDS,
            "disposition" => "ack_confirmed",
            "result" => status
        )
        .record(started.elapsed().as_secs_f64());

        result
    }
}

impl std::fmt::Debug for JetStreamDelivery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JetStreamDelivery")
            .field("subject", &self.subject)
            .field("payload_len", &self.payload.len())
            .field("has_headers", &self.headers.is_some())
            .field("stream_sequence", &self.info.stream_sequence)
            .field("consumer_sequence", &self.info.consumer_sequence)
            .field("pending", &self.info.pending)
            .finish()
    }
}

/// Backend abstraction used by the shared JetStream pull-consumer.
#[allow(async_fn_in_trait)]
pub trait JetStreamConsumerBackend: Send + Sync {
    /// Validates that the backend can reach the configured stream and consumer.
    async fn validate_startup(
        &self,
        config: &JetStreamConsumerConfig,
    ) -> Result<(), JetStreamError>;

    /// Pulls up to `max_messages` deliveries.
    async fn pull(
        &self,
        config: &JetStreamConsumerConfig,
        max_messages: usize,
        expires: Duration,
    ) -> Result<JetStreamDeliveryStream, JetStreamError>;
}

impl<T> JetStreamConsumerBackend for std::sync::Arc<T>
where
    T: JetStreamConsumerBackend,
{
    async fn validate_startup(
        &self,
        config: &JetStreamConsumerConfig,
    ) -> Result<(), JetStreamError> {
        (**self).validate_startup(config).await
    }

    async fn pull(
        &self,
        config: &JetStreamConsumerConfig,
        max_messages: usize,
        expires: Duration,
    ) -> Result<JetStreamDeliveryStream, JetStreamError> {
        (**self).pull(config, max_messages, expires).await
    }
}

/// Stream of JetStream deliveries produced by a bounded pull request.
pub type JetStreamDeliveryStream = BoxStream<'static, Result<JetStreamDelivery, JetStreamError>>;

/// Shared JetStream pull-consumer with reusable message helpers.
pub struct JetStreamPullConsumer<B = ContextConsumerBackend>
where
    B: JetStreamConsumerBackend,
{
    config: JetStreamConsumerConfig,
    backend: B,
}

impl JetStreamPullConsumer<ContextConsumerBackend> {
    /// Connects a consumer-backed JetStream context using the provided config.
    pub async fn connect(config: JetStreamConsumerConfig) -> Result<Self, JetStreamError> {
        if !config.enabled() {
            return Err(JetStreamError::Disabled);
        }

        let started = Instant::now();
        tracing::info!(
            stream = config.stream_name(),
            consumer = config.consumer_name(),
            "connecting JetStream consumer"
        );
        let result = async {
            connect_with_credentials(config.nats_url(), config.tls_policy(), config.credentials())
                .await
        }
        .await;
        if let Err(error) = &result {
            tracing::warn!(
                stream = config.stream_name(),
                consumer = config.consumer_name(),
                error = ?error,
                "consumer connect failed"
            );
        }
        let status = if result.is_ok() { "ok" } else { "error" };
        let _ = counter!(METRIC_NATS_CONSUMER_CONNECT_TOTAL, "result" => status);
        histogram!(
            METRIC_NATS_CONSUMER_CONNECT_DURATION_SECONDS,
            "result" => status
        )
        .record(started.elapsed().as_secs_f64());
        let client = result?;
        Ok(Self::from_client(client, config))
    }

    /// Constructs a consumer from an existing NATS client and validated config.
    ///
    /// `ack_timeout` is used both by the JetStream context and the explicit per-delivery
    /// ack timeout so stalled broker responses are fenced twice in the same timeout budget.
    pub fn from_client(client: async_nats::Client, config: JetStreamConsumerConfig) -> Self {
        let context = create_context(client, config.operation_timeout(), config.ack_timeout());
        Self {
            config,
            backend: ContextConsumerBackend::new(context),
        }
    }
}

impl<B> JetStreamPullConsumer<B>
where
    B: JetStreamConsumerBackend,
{
    /// Constructs a consumer from a validated config and a backend implementation.
    pub fn new_with_backend(config: JetStreamConsumerConfig, backend: B) -> Self {
        Self { config, backend }
    }

    /// Returns the validated config.
    pub const fn config(&self) -> &JetStreamConsumerConfig {
        &self.config
    }

    /// Validates startup access to the configured stream and consumer.
    pub async fn validate_startup(&self) -> Result<(), JetStreamError> {
        if !self.config.enabled() {
            return Err(JetStreamError::Disabled);
        }

        let started = Instant::now();
        tracing::info!(
            stream = self.config.stream_name(),
            consumer = self.config.consumer_name(),
            "validating JetStream consumer startup"
        );
        let result = self.backend.validate_startup(&self.config).await;
        if let Err(error) = &result {
            tracing::warn!(
                stream = self.config.stream_name(),
                consumer = self.config.consumer_name(),
                error = ?error,
                "consumer validate_startup failed"
            );
            let _ =
                counter!(METRIC_NATS_CONSUMER_VALIDATE_FAILURES_TOTAL, "reason" => "backend_error");
        }

        let status = if result.is_ok() { "ok" } else { "error" };
        let _ = counter!(METRIC_NATS_CONSUMER_VALIDATE_TOTAL, "result" => status);
        histogram!(
            METRIC_NATS_CONSUMER_VALIDATE_DURATION_SECONDS,
            "result" => status
        )
        .record(started.elapsed().as_secs_f64());

        result
    }

    /// Pulls a bounded batch of deliveries.
    pub async fn pull(
        &self,
        max_messages: usize,
        expires: Duration,
    ) -> Result<JetStreamDeliveryStream, JetStreamError> {
        if !self.config.enabled() {
            return Err(JetStreamError::Disabled);
        }

        if max_messages == 0 || expires.is_zero() {
            return Err(JetStreamError::InvalidConfiguration);
        }

        let started = Instant::now();
        tracing::info!(
            stream = self.config.stream_name(),
            consumer = self.config.consumer_name(),
            max_messages,
            "pulling JetStream deliveries"
        );
        let result = self.backend.pull(&self.config, max_messages, expires).await;
        if let Err(error) = &result {
            tracing::warn!(
                stream = self.config.stream_name(),
                consumer = self.config.consumer_name(),
                error = ?error,
                "consumer pull failed"
            );
            let _ = counter!(METRIC_NATS_CONSUMER_PULL_FAILURES_TOTAL, "reason" => "backend_error");
        }

        let status = if result.is_ok() { "ok" } else { "error" };
        let _ = counter!(
            METRIC_NATS_CONSUMER_PULL_TOTAL,
            "result" => status
        );
        histogram!(
            METRIC_NATS_CONSUMER_PULL_DURATION_SECONDS,
            "result" => status
        )
        .record(started.elapsed().as_secs_f64());
        result
    }
}

impl<B> std::fmt::Debug for JetStreamPullConsumer<B>
where
    B: JetStreamConsumerBackend,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JetStreamPullConsumer")
            .field("enabled", &self.config.enabled())
            .field("stream_name", &self.config.stream_name())
            .field("consumer_name", &self.config.consumer_name())
            .field("subject", &self.config.subject())
            .field(
                "operation_timeout_millis",
                &self.config.operation_timeout().as_millis(),
            )
            .field("ack_timeout_millis", &self.config.ack_timeout().as_millis())
            .field("max_ack_pending", &self.config.max_ack_pending())
            .finish()
    }
}

#[derive(Debug)]
/// Internal JetStream context-backed consumer cache and reconnect handling.
pub struct ContextConsumerBackend {
    context: async_nats::jetstream::Context,
    consumers: tokio::sync::RwLock<
        HashMap<ConsumerCacheKey, async_nats::jetstream::consumer::PullConsumer>,
    >,
    last_connection_state: tokio::sync::RwLock<async_nats::connection::State>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ConsumerCacheKey {
    stream_name: String,
    consumer_name: String,
    filter_subject: String,
}

impl ConsumerCacheKey {
    fn new(stream_name: &str, consumer_name: &str, filter_subject: &str) -> Self {
        Self {
            stream_name: stream_name.to_owned(),
            consumer_name: consumer_name.to_owned(),
            filter_subject: filter_subject.to_owned(),
        }
    }
}

impl ContextConsumerBackend {
    fn new(context: async_nats::jetstream::Context) -> Self {
        let connection_state = context.client().connection_state();
        Self {
            context,
            consumers: tokio::sync::RwLock::new(HashMap::new()),
            last_connection_state: tokio::sync::RwLock::new(connection_state),
        }
    }

    fn cache_key_for(config: &JetStreamConsumerConfig) -> ConsumerCacheKey {
        ConsumerCacheKey::new(
            config.stream_name(),
            config.consumer_name(),
            config.subject(),
        )
    }

    async fn invalidate(&self, cache_key: &ConsumerCacheKey) {
        let mut consumers = self.consumers.write().await;
        consumers.remove(cache_key);
    }

    async fn invalidate_all_if_reconnected(&self) {
        let current_state = self.context.client().connection_state();
        let should_clear = {
            let mut last_connection_state = self.last_connection_state.write().await;
            let should_clear = matches!(
                (&*last_connection_state, &current_state),
                (
                    &async_nats::connection::State::Disconnected,
                    &async_nats::connection::State::Connected
                ) | (
                    &async_nats::connection::State::Pending,
                    &async_nats::connection::State::Connected
                )
            );
            *last_connection_state = current_state;
            should_clear
        };

        if should_clear {
            let mut consumers = self.consumers.write().await;
            consumers.clear();
        }
    }

    async fn consumer(
        &self,
        config: &JetStreamConsumerConfig,
        force_refresh: bool,
    ) -> Result<async_nats::jetstream::consumer::PullConsumer, JetStreamError> {
        self.invalidate_all_if_reconnected().await;
        let cache_key = Self::cache_key_for(config);

        if !force_refresh {
            let consumers = self.consumers.read().await;
            if let Some(consumer) = consumers.get(&cache_key) {
                return Ok(consumer.clone());
            }
        }

        if force_refresh {
            let mut consumers = self.consumers.write().await;
            consumers.remove(&cache_key);
        }

        let stream = self
            .context
            .get_stream(config.stream_name().to_owned())
            .await
            .map_err(|_| JetStreamError::StreamLookupFailed)?;

        let created = stream
            .get_or_create_consumer(
                config.consumer_name(),
                async_nats::jetstream::consumer::pull::Config {
                    durable_name: Some(config.consumer_name().to_owned()),
                    filter_subject: config.subject().to_owned(),
                    ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                    ack_wait: config.ack_timeout(),
                    max_deliver: config.max_deliver(),
                    replay_policy: config.replay_policy(),
                    deliver_policy: config.deliver_policy(),
                    inactive_threshold: config.inactive_threshold(),
                    num_replicas: config.num_replicas(),
                    max_ack_pending: config.max_ack_pending(),
                    ..Default::default()
                },
            )
            .await
            .map_err(|_| JetStreamError::ConsumerInitializationFailed)?;

        let mut consumers = self.consumers.write().await;
        if let Some(existing) = consumers.get(&cache_key) {
            return Ok(existing.clone());
        }

        consumers.insert(cache_key, created.clone());
        Ok(created)
    }

    async fn pull_stream(
        &self,
        config: &JetStreamConsumerConfig,
        max_messages: usize,
        expires: Duration,
        force_refresh: bool,
    ) -> Result<JetStreamDeliveryStream, JetStreamError> {
        let consumer = self.consumer(config, force_refresh).await?;

        let messages = consumer
            .batch()
            .max_messages(max_messages)
            .expires(expires)
            .messages()
            .await
            .map_err(|_| JetStreamError::PullFailed)?;

        let ack_timeout = config.ack_timeout();
        let stream = stream::unfold(
            (messages, ack_timeout),
            |(mut messages, ack_timeout)| async move {
                let message = match messages.next().await {
                    Some(message) => message,
                    None => return None,
                };

                let message = match message {
                    Ok(message) => message,
                    Err(_) => {
                        let _ = counter!(
                            METRIC_NATS_CONSUMER_DELIVERY_FAILURE_TOTAL,
                            "result" => "error"
                        );
                        return Some((Err(JetStreamError::PullFailed), (messages, ack_timeout)));
                    }
                };

                let _ = counter!(METRIC_NATS_CONSUMER_DELIVERY_TOTAL, "result" => "ok");

                let subject = message.message.subject.as_str().to_owned();
                let payload = message.message.payload.clone();
                let headers = message.message.headers.clone();
                let info = message
                    .info()
                    .ok()
                    .map(|info| {
                        JetStreamDeliveryInfo::new(
                            Some(info.stream_sequence),
                            Some(info.consumer_sequence),
                            Some(info.pending),
                        )
                    })
                    .unwrap_or_default();

                Some((
                    Ok(JetStreamDelivery::new(
                        subject,
                        payload,
                        headers,
                        info,
                        message,
                        ack_timeout,
                    )),
                    (messages, ack_timeout),
                ))
            },
        );

        Ok(Box::pin(stream))
    }
}

impl JetStreamConsumerBackend for ContextConsumerBackend {
    async fn validate_startup(
        &self,
        config: &JetStreamConsumerConfig,
    ) -> Result<(), JetStreamError> {
        self.consumer(config, false).await.map(|_| ())
    }

    async fn pull(
        &self,
        config: &JetStreamConsumerConfig,
        max_messages: usize,
        expires: Duration,
    ) -> Result<JetStreamDeliveryStream, JetStreamError> {
        let cache_key = Self::cache_key_for(config);

        for attempt in 0..MAX_PULL_ATTEMPTS {
            let result = self
                .pull_stream(config, max_messages, expires, attempt > 0)
                .await;

            if let Ok(stream) = result {
                return Ok(stream);
            }

            self.invalidate(&cache_key).await;

            if attempt + 1 >= MAX_PULL_ATTEMPTS {
                return Err(JetStreamError::PullFailed);
            }
        }

        Err(JetStreamError::PullFailed)
    }
}

#[derive(Debug)]
struct ContextDeliveryAcker {
    message: async_nats::jetstream::Message,
    ack_timeout: Duration,
}

impl ContextDeliveryAcker {
    fn new(message: async_nats::jetstream::Message, ack_timeout: Duration) -> Self {
        Self {
            message,
            ack_timeout,
        }
    }
}

impl ContextDeliveryAcker {
    async fn acknowledge(
        &self,
        disposition: JetStreamAckDisposition,
        delay: Option<Duration>,
    ) -> Result<(), JetStreamError> {
        let disposition_label = match disposition {
            JetStreamAckDisposition::Ack => "ack",
            JetStreamAckDisposition::Nak => "nak",
            JetStreamAckDisposition::Term => "term",
        };
        // Keep the per-call timeout as a second fence in front of broker-side ack handling.
        let result = match disposition {
            JetStreamAckDisposition::Ack => {
                tokio::time::timeout(self.ack_timeout, self.message.ack())
                    .await
                    .map_err(|_| {
                        tracing::warn!(
                            disposition = disposition_label,
                            error = "consumer ack timeout"
                        );
                        JetStreamError::AcknowledgmentFailed
                    })?
            }
            JetStreamAckDisposition::Nak => {
                let ack_kind = async_nats::jetstream::AckKind::Nak(delay);
                tokio::time::timeout(self.ack_timeout, self.message.ack_with(ack_kind))
                    .await
                    .map_err(|_| {
                        tracing::warn!(
                            disposition = disposition_label,
                            error = "consumer ack_with timeout"
                        );
                        JetStreamError::AcknowledgmentFailed
                    })?
            }
            JetStreamAckDisposition::Term => tokio::time::timeout(
                self.ack_timeout,
                self.message.ack_with(async_nats::jetstream::AckKind::Term),
            )
            .await
            .map_err(|_| {
                tracing::warn!(
                    disposition = disposition_label,
                    error = "consumer ack term timeout"
                );
                JetStreamError::AcknowledgmentFailed
            })?,
        };

        result.map_err(|_| {
            tracing::warn!(
                disposition = disposition_label,
                error = "consumer ack request failed"
            );
            JetStreamError::AcknowledgmentFailed
        })
    }

    async fn acknowledge_confirmed(&self) -> Result<(), JetStreamError> {
        // Keep the per-call timeout as a second fence in front of broker-side ack confirmation.
        tokio::time::timeout(self.ack_timeout, self.message.double_ack())
            .await
            .map_err(|_| {
                tracing::warn!(error = "consumer ack_confirmed timeout");
                JetStreamError::AcknowledgmentFailed
            })?
            .map_err(|_| {
                tracing::warn!(error = "consumer ack_confirmed request failed");
                JetStreamError::AcknowledgmentFailed
            })
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use futures_util::StreamExt;

    use super::{JetStreamAckDisposition, JetStreamConsumerConfig, JetStreamPullConsumer};
    use crate::testing::{FakeJetStreamConsumerBackend, FakeJetStreamDelivery};
    use crate::{
        config::{JetStreamConsumerConfigInput, JetStreamPublisherConfig, JetStreamTlsPolicy},
        error::JetStreamError,
        publisher::JetStreamPublisher,
    };

    fn consumer_config() -> JetStreamConsumerConfig {
        let result = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
            enabled: true,
            nats_url: "nats://127.0.0.1:4222",
            stream_name: "updates",
            consumer_name: "updates_worker",
            subject: "updates.local",
            operation_timeout: Duration::from_secs(5),
            ack_timeout: Duration::from_secs(5),
            max_ack_pending: 32,
            max_deliver: 1,
            deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::All,
            replay_policy: async_nats::jetstream::consumer::ReplayPolicy::Instant,
            inactive_threshold: Duration::from_secs(30),
            num_replicas: 1,
            tls_policy: JetStreamTlsPolicy::Disabled,
        });
        assert!(result.is_ok());
        let Ok(config) = result else {
            unreachable!();
        };
        config
    }

    fn publisher_config(stream_name: &str, subject: &str) -> JetStreamPublisherConfig {
        let result = JetStreamPublisherConfig::new(
            true,
            "nats://127.0.0.1:4222",
            stream_name,
            subject,
            Duration::from_secs(5),
            1_024,
        );
        assert!(result.is_ok());
        let Ok(config) = result else {
            unreachable!();
        };
        config
    }

    fn should_run_nats_integration() -> bool {
        matches!(
            env::var("REALLYME_RUN_NATS_INTEGRATION").as_deref(),
            Ok("1")
        )
    }

    async fn connect_to_local_nats_for_integration()
    -> Result<async_nats::Client, async_nats::ConnectError> {
        async_nats::connect("nats://127.0.0.1:4222").await
    }

    #[tokio::test]
    async fn consumer_pull_rejects_zero_message_count() {
        let backend = Arc::new(FakeJetStreamConsumerBackend::default());
        let consumer = JetStreamPullConsumer::new_with_backend(consumer_config(), backend);

        let result = consumer.pull(0, Duration::from_millis(100)).await;
        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[tokio::test]
    async fn consumer_pull_rejects_zero_expire_timeout() {
        let backend = Arc::new(FakeJetStreamConsumerBackend::default());
        let consumer = JetStreamPullConsumer::new_with_backend(consumer_config(), backend);

        let result = consumer.pull(1, Duration::ZERO).await;
        assert!(matches!(result, Err(JetStreamError::InvalidConfiguration)));
    }

    #[tokio::test]
    async fn consumer_pull_returns_empty_stream_when_no_deliveries_queued() {
        let backend = Arc::new(FakeJetStreamConsumerBackend::default());
        let consumer = JetStreamPullConsumer::new_with_backend(consumer_config(), backend);

        let pull_result = consumer.pull(1, Duration::from_millis(100)).await;
        assert!(
            pull_result.is_ok(),
            "empty backend pull should return empty stream"
        );
        let Ok(mut deliveries) = pull_result else {
            return;
        };

        assert!(deliveries.next().await.is_none());
    }

    #[tokio::test]
    async fn consumer_pull_returns_fake_delivery_and_tracks_ack_disposition() {
        let backend = Arc::new(FakeJetStreamConsumerBackend::default());
        assert!(
            backend
                .push_delivery(FakeJetStreamDelivery::new(
                    "updates.local",
                    vec![1_u8, 2_u8]
                ))
                .is_ok(),
            "backend push should record fake delivery"
        );
        let consumer = JetStreamPullConsumer::new_with_backend(consumer_config(), backend.clone());

        let pull_result = consumer.pull(1, Duration::from_millis(100)).await;
        assert!(
            pull_result.is_ok(),
            "fake backend should return one delivery"
        );
        let Ok(mut deliveries) = pull_result else {
            return;
        };

        let maybe_delivery = deliveries.next().await;
        assert!(maybe_delivery.is_some(), "one delivery should be available");
        let Some(delivery) = maybe_delivery else {
            return;
        };
        assert!(delivery.is_ok(), "delivery should be valid");
        let Ok(delivery) = delivery else {
            return;
        };
        assert_eq!(delivery.payload(), &[1_u8, 2_u8]);
        assert!(delivery.term().await.is_ok());

        let dispositions_result = backend.dispositions();
        assert!(
            dispositions_result.is_ok(),
            "backend should report dispositions"
        );
        let Ok(dispositions) = dispositions_result else {
            return;
        };
        assert_eq!(dispositions, [JetStreamAckDisposition::Term]);
    }

    #[tokio::test]
    async fn consumer_pull_propagates_message_headers() {
        let backend = Arc::new(FakeJetStreamConsumerBackend::default());
        let mut headers = async_nats::HeaderMap::new();
        headers.append("x-reallyme-test", "header-propagation");

        assert!(
            backend
                .push_delivery(FakeJetStreamDelivery::with_headers(
                    "updates.local",
                    vec![9_u8],
                    headers,
                ))
                .is_ok(),
            "backend push should record fake delivery with headers"
        );
        let consumer = JetStreamPullConsumer::new_with_backend(consumer_config(), backend.clone());

        let pull_result = consumer.pull(1, Duration::from_millis(100)).await;
        assert!(
            pull_result.is_ok(),
            "fake backend should return one delivery"
        );
        let Ok(mut deliveries) = pull_result else {
            return;
        };

        let maybe_delivery = deliveries.next().await;
        assert!(maybe_delivery.is_some(), "one delivery should be available");
        let Some(delivery) = maybe_delivery else {
            return;
        };
        assert!(delivery.is_ok(), "delivery should be valid");
        let Ok(delivery) = delivery else {
            return;
        };
        assert!(delivery.headers().is_some());
        assert_eq!(delivery.payload(), &[9_u8]);
    }

    #[tokio::test]
    async fn real_publish_consume_and_ack_round_trip() {
        if !should_run_nats_integration() {
            println!(
                "SKIP: set REALLYME_RUN_NATS_INTEGRATION=1 to run this local JetStream integration test"
            );
            return;
        }

        let connected = connect_to_local_nats_for_integration().await;
        assert!(
            connected.is_ok(),
            "integration test requires nats-server at nats://127.0.0.1:4222"
        );
        let Ok(client) = connected else {
            return;
        };

        let nanos = SystemTime::now().duration_since(UNIX_EPOCH);
        assert!(nanos.is_ok(), "system time should be after unix epoch");
        let Ok(nanos) = nanos else {
            return;
        };
        let nanos = nanos.as_nanos();
        let stream_name = format!("rmn_{nanos}");
        let consumer_name = format!("rmn_consumer_{nanos}");
        let subject = format!("rmn.subject.{nanos}");
        let payload = b"from_real_jetstream";

        let context = async_nats::jetstream::new(client.clone());

        let stream_created = context
            .create_stream(async_nats::jetstream::stream::Config {
                name: stream_name.clone(),
                subjects: vec![subject.to_owned()],
                ..Default::default()
            })
            .await;
        assert!(
            stream_created.is_ok(),
            "stream create should succeed for integration test"
        );
        if stream_created.is_err() {
            return;
        }

        let stream_visible = context.get_stream(stream_name.to_owned()).await;
        assert!(
            stream_visible.is_ok(),
            "stream should be visible after create"
        );
        if stream_visible.is_err() {
            return;
        }

        let publisher = JetStreamPublisher::from_client(
            client.clone(),
            publisher_config(&stream_name, &subject),
        );
        let consumer_config = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
            enabled: true,
            nats_url: "nats://127.0.0.1:4222",
            stream_name: &stream_name,
            consumer_name: &consumer_name,
            subject: &subject,
            operation_timeout: Duration::from_secs(5),
            ack_timeout: Duration::from_secs(5),
            max_ack_pending: 128,
            max_deliver: 1,
            deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::All,
            replay_policy: async_nats::jetstream::consumer::ReplayPolicy::Instant,
            inactive_threshold: Duration::from_secs(30),
            num_replicas: 1,
            tls_policy: JetStreamTlsPolicy::Disabled,
        });
        assert!(consumer_config.is_ok(), "consumer config should validate");
        let Ok(consumer_config) = consumer_config else {
            return;
        };
        let consumer = JetStreamPullConsumer::from_client(client.clone(), consumer_config);

        let ack = publisher
            .publish_bytes(payload.as_ref(), None)
            .await
            .map_err(|_| ())
            .ok();
        assert!(ack.is_some(), "publish should succeed and return an ack");
        let Some(ack) = ack else {
            return;
        };
        assert_eq!(ack.stream_name(), stream_name.as_str());
        assert!(!ack.duplicate());
        assert!(!ack.stream_name().is_empty());

        let flush_result = client.flush().await;
        assert!(flush_result.is_ok(), "flush should succeed");
        if flush_result.is_err() {
            return;
        }
        let pull_result = consumer.pull(1, Duration::from_secs(5)).await;
        assert!(pull_result.is_ok(), "consumer pull should return stream");
        let Ok(mut deliveries) = pull_result else {
            return;
        };
        let maybe_delivery = deliveries.next().await;
        assert!(maybe_delivery.is_some(), "expected one delivery");
        let Some(delivery) = maybe_delivery else {
            return;
        };
        assert!(delivery.is_ok(), "delivery should be valid");
        let Ok(delivery) = delivery else {
            return;
        };
        assert_eq!(delivery.payload(), payload.as_ref());
        assert!(delivery.ack().await.is_ok());

        let _ = context.delete_stream(stream_name.to_owned()).await;
    }
}
