// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![cfg(feature = "testing")]

use std::env;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use metrics::with_local_recorder;
use metrics_util::debugging::{DebugValue, DebuggingRecorder};

use super::{
    JetStreamAckDisposition, JetStreamConsumerBackend, JetStreamConsumerConfig,
    JetStreamDeliveryStream, JetStreamPullConsumer, METRIC_NATS_CONSUMER_VALIDATE_FAILURES_TOTAL,
    METRIC_NATS_CONSUMER_VALIDATE_TOTAL,
};
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

struct RejectingConsumerBackend;

impl JetStreamConsumerBackend for RejectingConsumerBackend {
    async fn validate_startup(
        &self,
        _config: &JetStreamConsumerConfig,
    ) -> Result<(), JetStreamError> {
        Err(JetStreamError::ConnectFailed)
    }

    async fn pull(
        &self,
        _config: &JetStreamConsumerConfig,
        _max_messages: usize,
        _expires: Duration,
    ) -> Result<JetStreamDeliveryStream, JetStreamError> {
        Err(JetStreamError::PullFailed)
    }
}

#[test]
fn consumer_validation_failure_increments_both_outcome_counters() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let consumer =
        JetStreamPullConsumer::new_with_backend(consumer_config(), RejectingConsumerBackend);

    with_local_recorder(&recorder, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build");
        for _ in 0..3 {
            assert_eq!(
                runtime.block_on(consumer.validate_startup()),
                Err(JetStreamError::ConnectFailed)
            );
        }
    });

    let counters = snapshotter.snapshot().into_vec();
    for metric_name in [
        METRIC_NATS_CONSUMER_VALIDATE_FAILURES_TOTAL,
        METRIC_NATS_CONSUMER_VALIDATE_TOTAL,
    ] {
        assert!(counters.iter().any(|(key, _, _, value)| {
            key.key().name() == metric_name && *value == DebugValue::Counter(3)
        }));
    }
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
async fn consumer_pull_rejects_unbounded_batch_and_expiry() {
    let backend = Arc::new(FakeJetStreamConsumerBackend::default());
    let consumer = JetStreamPullConsumer::new_with_backend(consumer_config(), backend);

    for (max_messages, expires) in [
        (1_001, Duration::from_secs(1)),
        (1, Duration::from_secs(61)),
    ] {
        assert!(matches!(
            consumer.pull(max_messages, expires).await,
            Err(JetStreamError::InvalidConfiguration)
        ));
    }
}

#[tokio::test]
async fn consumer_delivery_progress_extends_ack_without_finishing_delivery() {
    let backend = Arc::new(FakeJetStreamConsumerBackend::default());
    backend
        .push_delivery(FakeJetStreamDelivery::new("updates.local", vec![1_u8]))
        .expect("fake delivery should be queued");
    let consumer = JetStreamPullConsumer::new_with_backend(consumer_config(), backend.clone());
    let mut deliveries = consumer
        .pull(1, Duration::from_secs(1))
        .await
        .expect("fake pull should succeed");
    let delivery = deliveries
        .next()
        .await
        .expect("one delivery should be present")
        .expect("fake delivery should be valid");

    delivery
        .in_progress()
        .await
        .expect("progress should succeed");
    delivery.ack().await.expect("terminal ack should succeed");
    assert_eq!(
        backend
            .dispositions()
            .expect("fake dispositions are readable"),
        [
            JetStreamAckDisposition::Progress,
            JetStreamAckDisposition::Ack
        ]
    );
}

#[tokio::test]
async fn consumer_pull_returns_empty_stream_when_no_deliveries_queued() {
    let backend = Arc::new(FakeJetStreamConsumerBackend::default());
    let consumer = JetStreamPullConsumer::new_with_backend(consumer_config(), backend);

    let mut deliveries = consumer
        .pull(1, Duration::from_millis(100))
        .await
        .expect("empty backend pull should return empty stream");

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

    let mut deliveries = consumer
        .pull(1, Duration::from_millis(100))
        .await
        .expect("fake backend should return one delivery");
    let delivery = deliveries
        .next()
        .await
        .expect("one delivery should be available")
        .expect("delivery should be valid");
    assert_eq!(delivery.payload(), &[1_u8, 2_u8]);
    assert!(delivery.term().await.is_ok());

    let dispositions = backend
        .dispositions()
        .expect("backend should report dispositions");
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

    let mut deliveries = consumer
        .pull(1, Duration::from_millis(100))
        .await
        .expect("fake backend should return one delivery");
    let delivery = deliveries
        .next()
        .await
        .expect("one delivery should be available")
        .expect("delivery should be valid");
    assert!(delivery.headers().is_some());
    assert_eq!(delivery.payload(), &[9_u8]);
}

#[tokio::test]
#[ignore = "requires a local NATS JetStream server"]
async fn real_publish_consume_and_ack_round_trip() {
    assert!(
        should_run_nats_integration(),
        "set REALLYME_RUN_NATS_INTEGRATION=1"
    );

    let client = connect_to_local_nats_for_integration()
        .await
        .expect("integration test requires nats-server at nats://127.0.0.1:4222");

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let stream_name = format!("rmn_{nanos}");
    let consumer_name = format!("rmn_consumer_{nanos}");
    let subject = format!("rmn.subject.{nanos}");
    let payload = b"from_real_jetstream";

    let context = async_nats::jetstream::new(client.clone());

    context
        .create_stream(async_nats::jetstream::stream::Config {
            name: stream_name.clone(),
            subjects: vec![subject.to_owned()],
            ..Default::default()
        })
        .await
        .expect("stream create should succeed for integration test");

    context
        .get_stream(stream_name.to_owned())
        .await
        .expect("stream should be visible after create");

    let publisher =
        JetStreamPublisher::from_client(client.clone(), publisher_config(&stream_name, &subject));
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
    })
    .expect("consumer config should validate");
    let consumer = JetStreamPullConsumer::from_client(client.clone(), consumer_config);

    let ack = publisher
        .publish_bytes(payload.as_ref(), None)
        .await
        .expect("publish should succeed and return an ack");
    assert_eq!(ack.stream_name(), stream_name.as_str());
    assert!(!ack.duplicate());
    assert!(!ack.stream_name().is_empty());

    client.flush().await.expect("flush should succeed");
    let mut deliveries = consumer
        .pull(1, Duration::from_secs(5))
        .await
        .expect("consumer pull should return stream");
    let delivery = deliveries
        .next()
        .await
        .expect("expected one delivery")
        .expect("delivery should be valid");
    assert_eq!(delivery.payload(), payload.as_ref());
    delivery.ack().await.expect("ack should succeed");

    context
        .delete_stream(stream_name.to_owned())
        .await
        .expect("integration stream should be deleted");
}
