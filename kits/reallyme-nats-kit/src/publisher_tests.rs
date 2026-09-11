// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::time::Duration;

use super::{JetStreamPublishAck, JetStreamPublisher};
use crate::testing::FakeJetStreamPublisherBackend;
use crate::{
    config::JetStreamPublisherConfig, error::JetStreamError,
    message_id::deterministic_message_id_for_payload,
};

const TEST_SUBJECT: &str = "test.jobs.requests.v1";

fn publisher_config() -> JetStreamPublisherConfig {
    let result = JetStreamPublisherConfig::new(
        true,
        "nats://127.0.0.1:4222",
        "search_spider",
        TEST_SUBJECT,
        Duration::from_secs(5),
        32,
    );
    assert!(result.is_ok());
    let Ok(config) = result else {
        unreachable!();
    };
    config
}

#[tokio::test]
async fn publisher_rejects_payloads_above_limit_before_backend_call() {
    let backend = Arc::new(FakeJetStreamPublisherBackend::default());
    let publisher = JetStreamPublisher::new_with_backend(publisher_config(), backend.clone());

    let result = publisher.publish_bytes(vec![0_u8; 33], Some("msg-1")).await;

    assert_eq!(result, Err(JetStreamError::PayloadTooLarge));
    let calls = backend.calls();
    assert!(calls.is_ok(), "backend calls should be available");
    let Ok(calls) = calls else {
        return;
    };
    assert!(calls.is_empty());
}

#[tokio::test]
async fn publisher_forwards_message_id_and_payload_to_backend() {
    let backend = Arc::new(FakeJetStreamPublisherBackend::default());
    assert!(
        backend.push_success_ack("search_spider", 7, false).is_ok(),
        "backend should queue synthetic success ack"
    );
    let publisher = JetStreamPublisher::new_with_backend(publisher_config(), backend.clone());

    let result = publisher
        .publish_bytes(vec![1_u8, 2_u8, 3_u8], Some("stable-id"))
        .await;
    assert!(result.is_ok());
    let Ok(result) = result else {
        unreachable!();
    };

    assert_eq!(result.sequence(), 7);
    let calls = backend.calls();
    assert!(calls.is_ok(), "backend calls should be available");
    let Ok(calls) = calls else {
        return;
    };
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].subject, TEST_SUBJECT);
    assert_eq!(calls[0].stream_name, "search_spider");
    assert_eq!(calls[0].message_id.as_deref(), Some("stable-id"));
    assert_eq!(calls[0].payload.as_ref(), &[1_u8, 2_u8, 3_u8]);
}

#[tokio::test]
async fn publisher_dedupe_helper_uses_deterministic_message_id() {
    let backend = Arc::new(FakeJetStreamPublisherBackend::default());
    assert!(
        backend.push_success_ack("search_spider", 9, true).is_ok(),
        "backend should queue synthetic success ack"
    );
    let publisher = JetStreamPublisher::new_with_backend(publisher_config(), backend.clone());
    let payload = vec![7_u8, 8_u8, 9_u8];

    let result = publisher.publish_bytes_deduplicated(payload.clone()).await;
    assert!(result.is_ok());

    let calls = backend.calls();
    assert!(calls.is_ok(), "backend calls should be available");
    let Ok(calls) = calls else {
        return;
    };
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].message_id.as_deref(),
        Some(deterministic_message_id_for_payload(TEST_SUBJECT, &payload).as_str())
    );
}

#[tokio::test]
async fn publisher_publish_without_backend_ack_fails() {
    let backend = Arc::new(FakeJetStreamPublisherBackend::default());
    let publisher = JetStreamPublisher::new_with_backend(publisher_config(), backend);

    let result = publisher.publish_bytes(vec![8_u8, 9_u8], None).await;
    assert_eq!(result, Err(JetStreamError::PublishNotAcknowledged));
}

#[test]
fn publish_ack_preserves_exact_stream_name() {
    let ack = JetStreamPublishAck::new("  search_spider  ", 11, false);
    assert!(ack.is_ok());
    let Ok(ack) = ack else {
        return;
    };

    assert_eq!(ack.stream_name(), "  search_spider  ");
}
