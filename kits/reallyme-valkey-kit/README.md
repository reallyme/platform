# reallyme-valkey-kit

First-class Valkey database connector for native ReallyMe services.

The kit owns:

- TLS-required-by-default endpoint configuration with redacted credentials.
- Programmatic and environment-prefix configuration shared by every app host.
- An eagerly connected, reconnecting multiplexed client.
- Readiness checks with a bounded, low-cardinality health report.
- Explicit connection, response, retry, concurrency, and pipeline bounds.
- Binary namespaced keys and values with fixed size ceilings.
- Generic typed commands and transactional/non-transactional pipelines for
  app-owned hashes, sets, sorted sets, lists, streams, and scripts.
- Mandatory, bounded TTLs for cache values and leases.
- Atomic acquire/release lease operations.
- Atomic counters whose expiration is set only when the counter is created.
- Typed, low-cardinality errors that never retain server diagnostics, keys,
  values, or credentials.

Application crates keep cache serialization and cache meaning in app-owned
adapters. For example, a search app owns how a response is encoded and uses
this kit only for Valkey connection and command behavior.

## Native composition

```rust,no_run
use reallyme_valkey_kit::{
    ValkeyConfig, ValkeyConnector, ValkeyResult,
};

async fn connect_valkey() -> ValkeyResult<ValkeyConnector> {
    let config = ValkeyConfig::from_env_prefix("EXAMPLE_SEARCH")?;
    let connector = ValkeyConnector::connect(&config).await?;
    connector.health_check().await?;
    Ok(connector)
}
```

Production composition should retain the default `RequireTls` policy.
`AllowPlaintextForDevelopment` exists only for explicit local test
composition. The config intentionally accepts host, port, database, username,
and password as separate fields instead of a URI so credentials are never
embedded in an endpoint string.

The connector uses RESP3 and an authenticated initial connection. A successful
`connect` call proves initial reachability; subsequent connection loss is
handled by the bounded reconnect policy configured on the connection manager.

For a prefix such as `EXAMPLE_SEARCH`, native composition reads:

```sh
export EXAMPLE_SEARCH_VALKEY_HOST="valkey.internal"
export EXAMPLE_SEARCH_VALKEY_PORT="6379"
export EXAMPLE_SEARCH_VALKEY_DATABASE="0"
export EXAMPLE_SEARCH_VALKEY_USERNAME="example-search"
export EXAMPLE_SEARCH_VALKEY_PASSWORD="..."
export EXAMPLE_SEARCH_VALKEY_KEY_PREFIX="reallyme:example-search"
export EXAMPLE_SEARCH_VALKEY_TLS_MODE="require"
# Optional private/internal CA; omit to use the maintained public root set.
export EXAMPLE_SEARCH_VALKEY_TLS_CA_PEM_PATH="/run/secrets/valkey-ca.pem"
export EXAMPLE_SEARCH_VALKEY_CONNECTION_TIMEOUT_MILLIS="3000"
export EXAMPLE_SEARCH_VALKEY_RESPONSE_TIMEOUT_MILLIS="2000"
export EXAMPLE_SEARCH_VALKEY_RETRY_ATTEMPTS="3"
export EXAMPLE_SEARCH_VALKEY_CONCURRENCY_LIMIT="1024"
export EXAMPLE_SEARCH_VALKEY_PIPELINE_BUFFER_SIZE="256"
```

`VALKEY_TLS_MODE` defaults to `require`. The only plaintext value is
`allow-plaintext-development`, intended for explicit local composition such as
the development Valkey bound to `127.0.0.1:6379`. A custom CA path is bounded,
read through a bounded adapter, and redacted from `Debug`; it cannot be combined
with plaintext mode.

## Generic command boundary

Apps can use `valkey_command` and `valkey_pipeline` for data structures that do
not have a kit convenience method. The command response is decoded into the
caller's chosen Redis-protocol type, while all driver/server diagnostics are
translated into `ValkeyError` before crossing the connector boundary.

Command names must remain compile-time application code. User input is allowed
only as an encoded argument. Application keys must be passed through
`ValkeyConnector::namespaced_key`; the connector deliberately does not attempt
to guess which arbitrary command arguments are keys. Pipelines are non-atomic
unless the app explicitly calls `atomic()`.

This generic boundary supports Valkey hashes, lists, sets, sorted sets, streams,
consumer groups, transactions, and fixed scripts without placing app schema or
payload semantics in the platform kit. The typed convenience methods remain the
preferred surface for ordinary expiring values, distributed leases, and fixed
window counters.

## Lease behavior

Acquire leases with `set_if_absent_with_ttl`. Release them with
`release_lease`, which uses one fixed server-side script to compare the token
and delete the key atomically. A caller must generate a cryptographically
random, single-use token and keep it in `ValkeyValue`; deleting a lease key
without comparing the token is not safe when ownership may have expired and
transferred.

## Counter behavior

`increment_with_ttl` increments and applies expiration in one server-side
operation. Expiration is applied only when the key is first created, producing
a fixed window rather than silently extending the window on each increment.
