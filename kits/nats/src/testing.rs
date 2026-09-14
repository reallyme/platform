// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Test fakes for JetStream publisher and consumer code.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use futures_util::stream;

use crate::{
    config::{JetStreamConsumerConfig, JetStreamPublisherConfig},
    consumer::{
        JetStreamAckDisposition, JetStreamConsumerBackend, JetStreamDelivery, JetStreamDeliveryInfo,
    },
    error::JetStreamError,
    publisher::{JetStreamPublishAck, JetStreamPublisherBackend},
};

/// Recorded fake publish invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakePublishCall {
    /// Recorded target subject.
    pub subject: String,
    /// Recorded expected stream name.
    pub stream_name: String,
    /// Recorded optional message identifier.
    pub message_id: Option<String>,
    /// Recorded payload bytes.
    pub payload: Bytes,
}

/// Fake publisher backend for tests.
#[derive(Default)]
pub struct FakeJetStreamPublisherBackend {
    calls: Mutex<Vec<FakePublishCall>>,
    results: Mutex<VecDeque<Result<JetStreamPublishAck, JetStreamError>>>,
}

impl FakeJetStreamPublisherBackend {
    /// Pushes a successful publish acknowledgment to be returned by the next publish.
    pub fn push_success_ack(
        &self,
        stream_name: &str,
        sequence: u64,
        duplicate: bool,
    ) -> Result<(), JetStreamError> {
        let mut results = self
            .results
            .lock()
            .map_err(|_| JetStreamError::SyncPrimitivePoisoned)?;
        let ack = JetStreamPublishAck::new(stream_name, sequence, duplicate)?;
        results.push_back(Ok(ack));
        Ok(())
    }

    /// Pushes a deterministic failure to be returned by the next publish.
    pub fn push_error(&self, error: JetStreamError) -> Result<(), JetStreamError> {
        let mut results = self
            .results
            .lock()
            .map_err(|_| JetStreamError::SyncPrimitivePoisoned)?;
        results.push_back(Err(error));
        Ok(())
    }

    /// Returns the recorded calls.
    pub fn calls(&self) -> Result<Vec<FakePublishCall>, JetStreamError> {
        self.calls
            .lock()
            .map_err(|_| JetStreamError::SyncPrimitivePoisoned)
            .map(|calls| calls.clone())
    }
}

impl JetStreamPublisherBackend for FakeJetStreamPublisherBackend {
    async fn validate_startup(
        &self,
        _config: &JetStreamPublisherConfig,
    ) -> Result<(), JetStreamError> {
        Ok(())
    }

    async fn publish(
        &self,
        config: &JetStreamPublisherConfig,
        payload: Bytes,
        message_id: Option<String>,
    ) -> Result<JetStreamPublishAck, JetStreamError> {
        {
            let mut calls = self
                .calls
                .lock()
                .map_err(|_| JetStreamError::SyncPrimitivePoisoned)?;
            calls.push(FakePublishCall {
                subject: config.subject().to_owned(),
                stream_name: config.stream_name().to_owned(),
                message_id,
                payload,
            });
        }

        let mut results = self
            .results
            .lock()
            .map_err(|_| JetStreamError::SyncPrimitivePoisoned)?;
        match results.pop_front() {
            Some(result) => result,
            None => Err(JetStreamError::PublishNotAcknowledged),
        }
    }
}

/// Fake delivery fixture for consumer tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeJetStreamDelivery {
    subject: String,
    headers: Option<async_nats::HeaderMap>,
    payload: Bytes,
}

impl FakeJetStreamDelivery {
    /// Constructs a fake delivery fixture.
    pub fn new(subject: &str, payload: impl Into<Bytes>) -> Self {
        Self {
            subject: subject.to_owned(),
            headers: None,
            payload: payload.into(),
        }
    }

    /// Constructs a fake delivery fixture with headers.
    pub fn with_headers(
        subject: &str,
        payload: impl Into<Bytes>,
        headers: async_nats::HeaderMap,
    ) -> Self {
        Self {
            subject: subject.to_owned(),
            headers: Some(headers),
            payload: payload.into(),
        }
    }
}

#[derive(Default)]
struct FakeConsumerState {
    queued: Mutex<VecDeque<FakeJetStreamDelivery>>,
    dispositions: Arc<Mutex<Vec<JetStreamAckDisposition>>>,
}

/// Fake consumer backend for tests.
#[derive(Default)]
pub struct FakeJetStreamConsumerBackend {
    state: Arc<FakeConsumerState>,
}

impl FakeJetStreamConsumerBackend {
    /// Enqueues a fake delivery to be returned on the next pull.
    pub fn push_delivery(&self, delivery: FakeJetStreamDelivery) -> Result<(), JetStreamError> {
        let mut queued = self
            .state
            .queued
            .lock()
            .map_err(|_| JetStreamError::SyncPrimitivePoisoned)?;
        queued.push_back(delivery);
        Ok(())
    }

    /// Returns all recorded acknowledgment dispositions.
    pub fn dispositions(&self) -> Result<Vec<JetStreamAckDisposition>, JetStreamError> {
        self.state
            .dispositions
            .lock()
            .map_err(|_| JetStreamError::SyncPrimitivePoisoned)
            .map(|values| values.clone())
    }
}

impl JetStreamConsumerBackend for FakeJetStreamConsumerBackend {
    async fn validate_startup(
        &self,
        _config: &JetStreamConsumerConfig,
    ) -> Result<(), JetStreamError> {
        Ok(())
    }

    async fn pull(
        &self,
        _config: &JetStreamConsumerConfig,
        max_messages: usize,
        _expires: Duration,
    ) -> Result<
        futures_util::stream::BoxStream<'static, Result<JetStreamDelivery, JetStreamError>>,
        JetStreamError,
    > {
        let mut deliveries: Vec<JetStreamDelivery> = Vec::with_capacity(max_messages);
        let mut queued = self
            .state
            .queued
            .lock()
            .map_err(|_| JetStreamError::SyncPrimitivePoisoned)?;

        for _ in 0..max_messages {
            let Some(delivery) = queued.pop_front() else {
                break;
            };
            deliveries.push(JetStreamDelivery::new_for_test(
                delivery.subject,
                delivery.payload,
                delivery.headers,
                JetStreamDeliveryInfo::default(),
                Arc::clone(&self.state.dispositions),
            ));
        }

        Ok(Box::pin(stream::iter(deliveries.into_iter().map(Ok))))
    }
}
