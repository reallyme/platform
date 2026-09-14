// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::num::NonZeroU64;
use std::time::Duration;

use reallyme_valkey_kit::{
    ValkeyConfig, ValkeyConfigInput, ValkeyConnector, ValkeyKey, ValkeyTimeToLive,
    ValkeyTransportSecurity, ValkeyValue, valkey_command, valkey_pipeline,
};

type StreamFields = Vec<(Vec<u8>, Vec<u8>)>;
type StreamRangeEntries = Vec<(String, StreamFields)>;

#[tokio::test]
#[ignore = "requires a local Valkey server on 127.0.0.1:6379"]
async fn cache_lease_and_counter_commands_are_atomic() {
    let config = ValkeyConfig::new(ValkeyConfigInput {
        host: "127.0.0.1".to_owned(),
        port: std::env::var("REALLYME_VALKEY_INTEGRATION_PORT")
            .map(|value| value.parse::<u16>().expect("valid test port"))
            .unwrap_or(6_379),
        key_prefix: "reallyme:integration".to_owned(),
        transport_security: ValkeyTransportSecurity::AllowPlaintextForDevelopment,
        ..ValkeyConfigInput::default()
    })
    .expect("integration configuration should be valid");
    let connector = ValkeyConnector::connect(&config)
        .await
        .expect("local Valkey should accept the connection");
    let report = connector
        .health_report()
        .await
        .expect("local Valkey should answer PING");
    assert!(!report.tls_enabled);
    assert_eq!(report.database, 0);

    let ttl =
        ValkeyTimeToLive::new(Duration::from_secs(30)).expect("integration TTL should be valid");
    let cache_key = ValkeyKey::new(b"cache".to_vec()).expect("cache key should be valid");
    let cache_value = ValkeyValue::new(b"payload".to_vec()).expect("value should be valid");
    connector
        .delete(&cache_key)
        .await
        .expect("cleanup should succeed");
    connector
        .set_with_ttl(&cache_key, &cache_value, ttl)
        .await
        .expect("cache set should succeed");
    let stored = connector
        .get(&cache_key)
        .await
        .expect("cache read should succeed")
        .expect("cache value should exist");
    assert_eq!(stored.as_bytes(), b"payload");

    let lease_key = ValkeyKey::new(b"lease".to_vec()).expect("lease key should be valid");
    let owner = ValkeyValue::new(b"owner-one".to_vec()).expect("owner token should be valid");
    let other = ValkeyValue::new(b"owner-two".to_vec()).expect("owner token should be valid");
    connector
        .delete(&lease_key)
        .await
        .expect("cleanup should succeed");
    assert!(
        connector
            .set_if_absent_with_ttl(&lease_key, &owner, ttl)
            .await
            .expect("first lease acquisition should succeed")
    );
    assert!(
        !connector
            .set_if_absent_with_ttl(&lease_key, &other, ttl)
            .await
            .expect("second lease acquisition should be rejected")
    );
    assert!(
        !connector
            .release_lease(&lease_key, &other)
            .await
            .expect("wrong-owner release should be evaluated")
    );
    assert!(
        connector
            .release_lease(&lease_key, &owner)
            .await
            .expect("owner release should succeed")
    );

    let counter_key = ValkeyKey::new(b"counter".to_vec()).expect("counter key should be valid");
    connector
        .delete(&counter_key)
        .await
        .expect("cleanup should succeed");
    let one = NonZeroU64::new(1).expect("one is non-zero");
    let two = NonZeroU64::new(2).expect("two is non-zero");
    assert_eq!(
        connector
            .increment_with_ttl(&counter_key, one, ttl)
            .await
            .expect("first increment should succeed"),
        1
    );
    assert_eq!(
        connector
            .increment_with_ttl(&counter_key, two, ttl)
            .await
            .expect("second increment should succeed"),
        3
    );

    // Existing zero values retain their original fixed window.
    let existing_zero = ValkeyValue::new(b"0".to_vec()).expect("zero");
    let short_ttl = ValkeyTimeToLive::new(Duration::from_secs(5)).expect("short TTL");
    connector
        .set_with_ttl(&counter_key, &existing_zero, short_ttl)
        .await
        .expect("zero counter");
    assert_eq!(
        connector
            .increment_with_ttl(&counter_key, one, ttl)
            .await
            .expect("increment"),
        1
    );
    let namespaced_counter = connector.namespaced_key(&counter_key).expect("namespace");
    let mut pttl = valkey_command("PTTL");
    pttl.arg(namespaced_counter.as_slice());
    let remaining: i64 = connector.query(&pttl).await.expect("TTL");
    assert!(remaining > 0 && remaining <= 5_000);

    let large = ValkeyValue::new(b"9007199254740992".to_vec()).expect("large counter");
    connector
        .set_with_ttl(&counter_key, &large, ttl)
        .await
        .expect("set large counter");
    assert_eq!(
        connector
            .increment_with_ttl(&counter_key, one, ttl)
            .await
            .expect("exact increment"),
        9_007_199_254_740_993
    );
    let largest = ValkeyValue::new(b"9223372036854775806".to_vec()).expect("large counter");
    connector
        .set_with_ttl(&counter_key, &largest, ttl)
        .await
        .expect("set largest counter");
    assert_eq!(
        connector
            .increment_with_ttl(&counter_key, one, ttl)
            .await
            .expect("exact increment"),
        9_223_372_036_854_775_807
    );
    assert!(
        connector
            .increment_with_ttl(&counter_key, one, ttl)
            .await
            .is_err()
    );

    let generic_key = ValkeyKey::new(b"generic".to_vec()).expect("generic key should be valid");
    let namespaced_key = connector
        .namespaced_key(&generic_key)
        .expect("namespace construction should succeed");
    let mut set_command = valkey_command("SET");
    set_command
        .arg(namespaced_key.as_slice())
        .arg(b"generic-value")
        .arg("PX")
        .arg(30_000_u64);
    connector
        .query::<()>(&set_command)
        .await
        .expect("generic typed command should succeed");

    let mut pipeline = valkey_pipeline();
    pipeline
        .cmd("GET")
        .arg(namespaced_key.as_slice())
        .cmd("PTTL")
        .arg(namespaced_key.as_slice());
    let (generic_value, generic_ttl): (Vec<u8>, i64) = connector
        .query_pipeline(&pipeline)
        .await
        .expect("generic typed pipeline should succeed");
    assert_eq!(generic_value, b"generic-value");
    assert!(generic_ttl > 0);

    let stream_key = ValkeyKey::new(b"stream".to_vec()).expect("stream key should be valid");
    let namespaced_stream_key = connector
        .namespaced_key(&stream_key)
        .expect("stream namespace construction should succeed");
    connector
        .delete(&stream_key)
        .await
        .expect("stream cleanup should succeed");
    let mut append_stream = valkey_command("XADD");
    append_stream
        .arg(namespaced_stream_key.as_slice())
        .arg("MAXLEN")
        .arg("~")
        .arg(100_u64)
        .arg("*")
        .arg("event")
        .arg(b"created");
    let stream_id: String = connector
        .query(&append_stream)
        .await
        .expect("stream append should succeed");
    assert!(!stream_id.is_empty());

    let mut range_stream = valkey_command("XRANGE");
    range_stream
        .arg(namespaced_stream_key.as_slice())
        .arg("-")
        .arg("+")
        .arg("COUNT")
        .arg(10_u64);
    let entries: StreamRangeEntries = connector
        .query(&range_stream)
        .await
        .expect("stream range should decode into a typed response");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, stream_id);
    assert_eq!(entries[0].1, vec![(b"event".to_vec(), b"created".to_vec())]);

    connector
        .delete(&cache_key)
        .await
        .expect("cleanup should succeed");
    connector
        .delete(&counter_key)
        .await
        .expect("cleanup should succeed");
    connector
        .delete(&generic_key)
        .await
        .expect("cleanup should succeed");
    connector
        .delete(&stream_key)
        .await
        .expect("stream cleanup should succeed");
}
