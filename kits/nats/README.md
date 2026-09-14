# reallyme-nats-kit

Reusable NATS and JetStream transport primitives for ReallyMe services and queue
consumers.

This kit owns:
- Validated NATS connection, stream, consumer, and subject configuration.
- NATS connection construction with typed credentials and a TLS policy.
- JetStream context construction with explicit request and ack timeouts.
- A JetStream publisher: bounded payloads, publish acks, deterministic dedupe.
- A durable JetStream pull consumer: consumer create/caching, reconnect
  invalidation, delivery ack/nak/term.
- Small fakes for app and queue-consumer tests.

This kit does not own:
- JetStream stream provisioning. The stream must already exist; the kit looks
  it up and creates only the durable consumer.
- Subject names or payload schemas. Payloads are opaque bytes and applications
  own their event contracts.
- App business logic, product message routing, or service runtime wiring.
- Dashboard or public API contracts.

## Module map

- [src/config.rs](src/config.rs): URL/credential/TLS validation, connection and context helpers
- [src/publisher.rs](src/publisher.rs): JetStream publisher and publish-ack type
- [src/consumer.rs](src/consumer.rs): durable pull consumer, delivery stream, ack flows
- [src/message_id.rs](src/message_id.rs): deterministic message-id helpers for dedupe
- [src/error.rs](src/error.rs): `JetStreamError` and the low-cardinality `JetStreamErrorReason`
- [src/testing.rs](src/testing.rs): in-memory fake publisher/consumer backends

## Publishing

`JetStreamPublisherConfig::new` validates the URL, stream, and subject, and
derives the TLS policy from the URL scheme. Configs return `JetStreamError`;
model the error rather than unwrapping (the crate denies `unwrap`/`expect` in
non-test code).

```rust
use std::time::Duration;
use reallyme_nats_kit::config::JetStreamPublisherConfig;
use reallyme_nats_kit::publisher::JetStreamPublisher;
use reallyme_nats_kit::error::JetStreamError;

const JOB_REQUESTS_SUBJECT: &str = "example.jobs.requests.v1";

async fn publish(payload: Vec<u8>) -> Result<(), JetStreamError> {
    let config = JetStreamPublisherConfig::new(
        true,                               // enabled
        "nats://127.0.0.1:4222",
        "EXAMPLE_JOBS",                     // pre-existing stream
        JOB_REQUESTS_SUBJECT,
        Duration::from_secs(5),             // publish + ack timeout
        64 * 1024,                          // max payload bytes
    )?;

    let publisher = JetStreamPublisher::connect(config).await?;
    publisher.validate_startup().await?;    // confirms the stream is reachable

    // Explicit dedupe key:
    let ack = publisher.publish_bytes(payload.clone(), Some("request-42")).await?;
    if ack.duplicate() {
        // JetStream already had this message id; safe to ignore.
    }

    // Or let the kit derive a deterministic id from subject + payload bytes:
    let _ack = publisher.publish_bytes_deduplicated(payload).await?;
    Ok(())
}
```

Payloads accept `impl Into<Bytes>` (`Vec<u8>`, `Bytes`, `&'static [u8]`).
Payloads larger than `max_payload_bytes` are rejected with
`JetStreamError::PayloadTooLarge` before any network call.

## Consuming

The consumer creates and caches the durable consumer (keyed by stream, durable
name, and filter subject) and invalidates the cache on reconnect. `pull`
returns a lazy stream so payloads are not all buffered at once.

```rust
use std::time::Duration;
use futures_util::StreamExt;
use async_nats::jetstream::consumer::{DeliverPolicy, ReplayPolicy};
use reallyme_nats_kit::config::{JetStreamConsumerConfig, JetStreamConsumerConfigInput};
use reallyme_nats_kit::consumer::JetStreamPullConsumer;
use reallyme_nats_kit::error::JetStreamError;

const JOB_REQUESTS_SUBJECT: &str = "example.jobs.requests.v1";

async fn consume() -> Result<(), JetStreamError> {
    let config = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
        enabled: true,
        nats_url: "nats://127.0.0.1:4222",
        stream_name: "EXAMPLE_JOBS",
        consumer_name: "job-consumer",      // durable name
        subject: JOB_REQUESTS_SUBJECT,       // filter subject
        operation_timeout: Duration::from_secs(5),
        ack_timeout: Duration::from_secs(30),
        max_ack_pending: 256,               // -1 for unlimited
        max_deliver: 5,
        deliver_policy: DeliverPolicy::All,
        replay_policy: ReplayPolicy::Instant,
        inactive_threshold: Duration::from_secs(300),
        num_replicas: 1,
        tls_policy: Default::default(),
    })?;

    let consumer = JetStreamPullConsumer::connect(config).await?;
    consumer.validate_startup().await?;

    let mut deliveries = consumer.pull(64, Duration::from_secs(2)).await?;
    while let Some(delivery) = deliveries.next().await {
        let delivery = delivery?;
        match handle(delivery.payload()) {
            Ok(()) => delivery.ack().await?,
            Err(retryable) if retryable => delivery.nak().await?,
            Err(_) => delivery.term().await?, // poison message, stop redelivery
        }
    }
    Ok(())
}

fn handle(_payload: &[u8]) -> Result<(), bool> { Ok(()) }
```

`ack_confirmed()` waits for the broker to confirm the ack;
`nak_with_delay(Duration)` requests delayed redelivery. `delivery.info()`
exposes stream/consumer sequence and pending counts.

## Credentials and TLS

Credentials in the raw URL are rejected. Pass them through the typed enum and
build the config with `*_with_credentials`:

```rust
use std::time::Duration;
use reallyme_nats_kit::config::{
    JetStreamCredentials, JetStreamPublisherConfig, JetStreamPublisherConfigInput,
    JetStreamTlsPolicy,
};
use reallyme_nats_kit::error::JetStreamError;

fn secure_config(token: String) -> Result<JetStreamPublisherConfig, JetStreamError> {
    JetStreamPublisherConfig::new_with_credentials(
        JetStreamPublisherConfigInput {
            enabled: true,
            nats_url: "tls://nats.internal:4222",
            stream_name: "REALLYME_SPIDER",
            subject: "reallyme.spider.requests.v1",
            publish_timeout: Duration::from_secs(5),
            max_payload_bytes: 64 * 1024,
            tls_policy: JetStreamTlsPolicy::Required,
        },
        JetStreamCredentials::Token(token),
    )
}
```

`JetStreamCredentials` supports `None`, `Token`, `Jwt { jwt, nkey_seed }`,
`NKey`, and `CredentialsFile(PathBuf)`. `JetStreamTlsPolicy::Required` rejects
non-TLS URLs; `Disabled` allows cleartext for local/test; `Optional` (default)
is derived from the URL scheme by `JetStreamPublisherConfig::new`. Credentials
and the URL userinfo are redacted from `Debug`.

## Testing

`testing.rs` provides in-memory backends so app and queue-consumer tests need no
broker. Inject them with `new_with_backend`:

```rust
use reallyme_nats_kit::publisher::JetStreamPublisher;
use reallyme_nats_kit::testing::FakeJetStreamPublisherBackend;

let backend = FakeJetStreamPublisherBackend::default();
backend.push_success_ack("REALLYME_SPIDER", 1, false).unwrap();
let publisher = JetStreamPublisher::new_with_backend(config, backend);
// ... assert on backend.calls()
```

`FakeJetStreamConsumerBackend` accepts queued `FakeJetStreamDelivery` values and
records the ack/nak/term disposition of each, so consumer logic can be tested
deterministically. A fake publisher with no queued result returns an explicit
error rather than a synthetic success.

## Integration tests

Unit tests always run. The JetStream round-trip test
`real_publish_consume_and_ack_round_trip` is gated behind
`REALLYME_RUN_NATS_INTEGRATION=1` and fails fast if the server is unreachable.

Start a local NATS server with JetStream enabled:

```console
nats-server --jetstream --port 4222
```

Then run the integration-enabled suite:

```console
REALLYME_RUN_NATS_INTEGRATION=1 cargo test -p reallyme-nats-kit
```

## Observability

Connect, startup-validate, publish, pull, and ack paths emit `tracing` spans
and `metrics` counters/histograms. Error labels use the low-cardinality
`JetStreamErrorReason`, never raw error strings, so dashboards and alerts stay
bounded.

## Quality gates

```bash
cargo fmt --check
cargo clippy -p reallyme-nats-kit --all-targets -- -D warnings
cargo test -p reallyme-nats-kit
```
