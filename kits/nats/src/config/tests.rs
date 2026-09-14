// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use async_nats::jetstream::consumer::{DeliverPolicy, ReplayPolicy};
use secrecy::{ExposeSecret, SecretString};

use super::{
    JetStreamConsumerConfig, JetStreamConsumerConfigInput, JetStreamCredentials, JetStreamError,
    JetStreamPublisherConfig, JetStreamPublisherConfigInput, JetStreamTlsPolicy,
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
