# reallyme-nats-kit

NATS connections, JetStream publishers, and durable pull consumers.

Provision the JetStream stream before connecting; the kit creates durable
consumers but does not provision streams. Applications choose subjects and
payload formats.

## Publishing

`JetStreamPublisherConfig::new` validates the URL, stream, subject, timeout,
and payload limit. Its convenience constructor allows plaintext for loopback
endpoints. Use `Required` TLS for non-local connections. Explicit plaintext
connections ignore server-advertised addresses.

```rust
use reallyme_nats_kit::config::JetStreamPublisherConfig;
use reallyme_nats_kit::error::JetStreamError;
use reallyme_nats_kit::publisher::JetStreamPublisher;
use std::time::Duration;

const JOB_REQUESTS_SUBJECT: &str = "example.jobs.requests.v1";

async fn publish(
    payload: Vec<u8>,
    message_id: &str,
) -> Result<reallyme_nats_kit::publisher::JetStreamPublishAck, JetStreamError> {
    let config = JetStreamPublisherConfig::new(
        true,
        "nats://127.0.0.1:4222",
        "EXAMPLE_JOBS",
        JOB_REQUESTS_SUBJECT,
        Duration::from_secs(5),
        64 * 1024,
    )?;
    let publisher = JetStreamPublisher::connect(config).await?;
    publisher.validate_startup().await?;
    publisher.publish_bytes(payload, Some(message_id)).await
}
```

Payloads accept `impl Into<Bytes>` (`Vec<u8>`, `Bytes`, `&'static [u8]`).
Payloads larger than `max_payload_bytes` are rejected with
`JetStreamError::PayloadTooLarge` before any network call.
Use an operation-specific message ID for deduplication. The acknowledgement's
`duplicate()` flag reports a broker duplicate. `publish_bytes_deduplicated`
derives an ID from subject and payload bytes.

## Consuming

The consumer creates and caches the durable consumer (keyed by stream, durable
name, and filter subject) and invalidates the cache on reconnect. `pull`
returns a lazy stream so payloads are not all buffered at once.

```rust
use async_nats::jetstream::consumer::{DeliverPolicy, ReplayPolicy};
use futures_util::StreamExt;
use reallyme_nats_kit::config::{
    JetStreamConsumerConfig, JetStreamConsumerConfigInput, JetStreamTlsPolicy,
};
use reallyme_nats_kit::consumer::JetStreamPullConsumer;
use reallyme_nats_kit::error::JetStreamError;
use std::time::Duration;

const JOB_REQUESTS_SUBJECT: &str = "example.jobs.requests.v1";

enum DeliveryDisposition {
    Acknowledge,
    Retry,
    Reject,
}

async fn consume(on_payload: impl Fn(&[u8]) -> DeliveryDisposition) -> Result<(), JetStreamError> {
    let config = JetStreamConsumerConfig::new(JetStreamConsumerConfigInput {
        enabled: true,
        nats_url: "nats://127.0.0.1:4222",
        stream_name: "EXAMPLE_JOBS",
        consumer_name: "job-consumer", // durable name
        subject: JOB_REQUESTS_SUBJECT, // filter subject
        operation_timeout: Duration::from_secs(5),
        ack_timeout: Duration::from_secs(30),
        max_ack_pending: 256,
        max_deliver: 5,
        deliver_policy: DeliverPolicy::All,
        replay_policy: ReplayPolicy::Instant,
        inactive_threshold: Duration::from_secs(300),
        num_replicas: 1,
        tls_policy: JetStreamTlsPolicy::Disabled, // explicit loopback development transport
    })?;

    let consumer = JetStreamPullConsumer::connect(config).await?;
    consumer.validate_startup().await?;

    let mut deliveries = consumer.pull(64, Duration::from_secs(2)).await?;
    while let Some(delivery) = deliveries.next().await {
        let delivery = delivery?;
        match on_payload(delivery.payload()) {
            DeliveryDisposition::Acknowledge => delivery.ack().await?,
            DeliveryDisposition::Retry => delivery.nak().await?,
            DeliveryDisposition::Reject => delivery.term().await?,
        }
    }
    Ok(())
}
```

`ack_confirmed()` waits for the broker to confirm the ack;
`nak_with_delay(Duration)` requests delayed redelivery. `delivery.info()`
exposes stream/consumer sequence and pending counts.

## Credentials and TLS

Credentials in the raw URL are rejected. Pass them through the typed enum and
build the config with `*_with_credentials`:

```rust
use reallyme_nats_kit::config::{
    JetStreamCredentials, JetStreamPublisherConfig, JetStreamPublisherConfigInput,
    JetStreamTlsPolicy,
};
use reallyme_nats_kit::error::JetStreamError;
use secrecy::SecretString;
use std::time::Duration;

fn secure_config(token: SecretString) -> Result<JetStreamPublisherConfig, JetStreamError> {
    JetStreamPublisherConfig::new_with_credentials(
        JetStreamPublisherConfigInput {
            enabled: true,
            nats_url: "tls://nats.internal:4222",
            stream_name: "EXAMPLE_JOBS",
            subject: "example.jobs.requests.v1",
            publish_timeout: Duration::from_secs(5),
            max_payload_bytes: 64 * 1024,
            tls_policy: JetStreamTlsPolicy::Required,
        },
        JetStreamCredentials::Token(token),
    )
}
```

`JetStreamCredentials` supports `None`, `Token`, `Jwt { jwt, nkey_seed }`,
`NKey`, and `CredentialsFile(PathBuf)`. Token, JWT, and NKey material use
`secrecy::SecretString`. `JetStreamTlsPolicy` defaults to `Required`;
`Optional` is a compatibility spelling that also requires TLS. `Disabled`
allows explicit plaintext local development connections. The publisher's
convenience constructor selects `Disabled` for loopback plaintext endpoints
and `Required` for TLS or non-loopback endpoints. Credentials and URL userinfo
are redacted from `Debug`.

The default connector uses operating-system certificate roots, falling back
to bundled public roots only if the host store is empty. For a private CA,
`connect_with_credentials_and_custom_tls_roots` accepts a nonempty
`rustls::RootCertStore` and trusts only those roots. It requires a TLS policy
other than `Disabled`. Pass the returned client to `JetStreamPublisher::from_client`
or `JetStreamPullConsumer::from_client` with the validated config.

## Testing

Enable the `testing` feature to use in-memory backends without a broker. Inject
them with `new_with_backend`:

```rust
use reallyme_nats_kit::config::JetStreamPublisherConfig;
use reallyme_nats_kit::error::JetStreamError;
use reallyme_nats_kit::publisher::JetStreamPublisher;
use reallyme_nats_kit::testing::FakeJetStreamPublisherBackend;

async fn publish_with_fake(config: JetStreamPublisherConfig) -> Result<(), JetStreamError> {
    let backend = FakeJetStreamPublisherBackend::default();
    backend.push_success_ack("EXAMPLE_JOBS", 1, false)?;
    let publisher = JetStreamPublisher::new_with_backend(config, backend);
    let ack = publisher
        .publish_bytes(vec![1_u8], Some("request-42"))
        .await?;
    assert_eq!(ack.sequence(), 1);
    Ok(())
}
```

`FakeJetStreamConsumerBackend` queues deliveries and records ack/nak/term
results. Queue publisher results explicitly; an empty fake returns an error.

## Integration tests

The JetStream round-trip test `real_publish_consume_and_ack_round_trip` is
ignored by ordinary test runs. It requires both `REALLYME_RUN_NATS_INTEGRATION=1`
and explicit ignored-test selection, and fails if the server is unreachable.

Start a local NATS server with JetStream enabled:

```console
nats-server --jetstream --port 4222
```

Then run the integration-enabled suite:

```console
REALLYME_RUN_NATS_INTEGRATION=1 cargo test -p reallyme-nats-kit --features testing --lib \
  consumer::tests::real_publish_consume_and_ack_round_trip -- --ignored --exact
```

The kit emits tracing spans and metrics for connections, publishing, pulling,
and acknowledgements. Failure labels use `JetStreamErrorReason`. For workspace
checks, see [Building and testing](../../docs/guides/validation.md).
