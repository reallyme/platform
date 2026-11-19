# reallyme-valkey-kit

Valkey connections, namespaced keys, expiring values, leases, and counters.

TLS connections preserve a rustls process crypto provider selected by the host.
If none is installed, the kit installs ring because the Redis driver uses the
process provider to construct its TLS client. Connection setup returns a typed
error if no provider is available after installation.

## Connect

```rust,no_run
use reallyme_valkey_kit::{ValkeyConfig, ValkeyConnector, ValkeyResult};

async fn connect_valkey() -> ValkeyResult<ValkeyConnector> {
    let config = ValkeyConfig::from_env_prefix("EXAMPLE_SEARCH")?;
    let connector = ValkeyConnector::connect(&config).await?;
    connector.health_check().await?;
    Ok(connector)
}
```

Keep `RequireTls` in production. `AllowPlaintextForDevelopment` permits explicit
local test connections. Supply host, port, database, username, and password as
separate fields.

The connector uses RESP3 and connects before returning. Configure bounded
connection/response timeouts, retries, concurrency, and pipeline size.

For a prefix such as `EXAMPLE_SEARCH`, native composition reads:

```sh
export EXAMPLE_SEARCH_VALKEY_HOST="valkey.internal"
export EXAMPLE_SEARCH_VALKEY_PORT="6379"
export EXAMPLE_SEARCH_VALKEY_DATABASE="0"
export EXAMPLE_SEARCH_VALKEY_USERNAME="example-search"
export EXAMPLE_SEARCH_VALKEY_PASSWORD="..."
export EXAMPLE_SEARCH_VALKEY_KEY_PREFIX="reallyme-example-search"
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
`ValkeyConnector::namespaced_key`; the connector does not attempt
to guess which arbitrary command arguments are keys. Pipelines are non-atomic
unless the app explicitly calls `atomic()`.

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
