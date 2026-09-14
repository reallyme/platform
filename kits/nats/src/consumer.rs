// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::stream::BoxStream;
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

#[path = "consumer/context_backend.rs"]
mod context_backend;

pub use context_backend::ContextConsumerBackend;
use context_backend::ContextDeliveryAcker;

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

#[path = "consumer/debug.rs"]
mod debug;

#[cfg(test)]
#[path = "consumer/tests.rs"]
mod tests;
