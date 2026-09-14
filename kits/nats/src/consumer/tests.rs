// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
