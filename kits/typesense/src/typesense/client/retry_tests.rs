// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::TypesenseClient;
use crate::typesense::{TypesenseConfig, TypesenseEndpoint};
use secrecy::SecretString;
use std::time::Duration;

#[test]
fn server_retry_after_and_jitter_cannot_exceed_local_policy() {
    let config = TypesenseConfig::new(
        TypesenseEndpoint::parse("https://typesense.invalid").expect("endpoint"),
        SecretString::from("key"),
        Duration::from_secs(1),
    )
    .expect("config")
    .with_retry_max_delay(Duration::from_secs(2))
    .with_retry_initial_delay(Duration::from_secs(2))
    .with_retry_jitter_percent(100);
    let http_client = reqwest::Client::builder()
        .tls_backend_preconfigured(crate::typesense::connector::tls_config().expect("test TLS"))
        .build()
        .expect("test HTTP client");
    let client = TypesenseClient::new(http_client, config);
    let first = client.calculate_backoff_delay(32, None);
    assert!((0..32).any(|_| client.calculate_backoff_delay(32, None) != first));
    for attempt in [0, 1, 32, 255] {
        assert!(
            client.calculate_backoff_delay(attempt, Some(Duration::MAX)) <= Duration::from_secs(2)
        );
        assert!(client.calculate_backoff_delay(attempt, None) <= Duration::from_secs(2));
    }
}
