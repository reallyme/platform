// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! JetStream context-backed consumer caching, pulls, and acknowledgments.

use std::collections::HashMap;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream;
use metrics::counter;

use super::{
    JetStreamAckDisposition, JetStreamConsumerBackend, JetStreamDelivery, JetStreamDeliveryInfo,
    JetStreamDeliveryStream, MAX_PULL_ATTEMPTS, METRIC_NATS_CONSUMER_DELIVERY_FAILURE_TOTAL,
    METRIC_NATS_CONSUMER_DELIVERY_TOTAL,
};
use crate::config::JetStreamConsumerConfig;
use crate::error::JetStreamError;

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
    pub(super) fn new(context: async_nats::jetstream::Context) -> Self {
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
pub(super) struct ContextDeliveryAcker {
    message: async_nats::jetstream::Message,
    ack_timeout: Duration,
}

impl ContextDeliveryAcker {
    pub(super) fn new(message: async_nats::jetstream::Message, ack_timeout: Duration) -> Self {
        Self {
            message,
            ack_timeout,
        }
    }
}

impl ContextDeliveryAcker {
    pub(super) async fn acknowledge(
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

    pub(super) async fn acknowledge_confirmed(&self) -> Result<(), JetStreamError> {
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
