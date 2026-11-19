# reallyme-postgres-kit

PostgreSQL connection pools, transactions, readiness checks, and migration locks
using `bb8` and `tokio-postgres`. TLS with certificate validation is the default.

## Use from a native adapter

Construct the pool in a native host or database adapter:

```rust,no_run
use reallyme_postgres_kit::{PostgresConfig, PostgresPool, PostgresResult};

async fn connect_postgres() -> PostgresResult<PostgresPool> {
    let config = PostgresConfig::from_env_prefix("EXAMPLE_SERVICE")?;
    let pool = PostgresPool::connect(&config).await?;
    pool.health_check().await?;
    Ok(pool)
}
```

`PostgresPool::connect` establishes the configured minimum connections before
returning. The minimum defaults to one; zero is rejected.

The environment prefix is restricted to uppercase ASCII letters, digits, and
underscores. Environment values with invalid encoding fail closed rather than
silently selecting a default.

## Configuration

For a prefix such as `EXAMPLE_SERVICE`, the kit reads:

```sh
export EXAMPLE_SERVICE_POSTGRES_URI="postgres://user:password@host:5432/database"
export EXAMPLE_SERVICE_POSTGRES_APPLICATION_NAME="example-service"
export EXAMPLE_SERVICE_POSTGRES_MAX_POOL_SIZE="16"
export EXAMPLE_SERVICE_POSTGRES_MIN_POOL_SIZE="1"
export EXAMPLE_SERVICE_POSTGRES_CONNECTION_TIMEOUT_MILLIS="5000"
export EXAMPLE_SERVICE_POSTGRES_TLS_MODE="require"
# Set this for a private/internal PostgreSQL certificate authority. When set,
# the adapter trusts only this CA for PostgreSQL.
export EXAMPLE_SERVICE_POSTGRES_TLS_CA_PEM_PATH="/run/secrets/postgres-ca.pem"
export EXAMPLE_SERVICE_POSTGRES_STATEMENT_TIMEOUT_MILLIS="10000"
export EXAMPLE_SERVICE_POSTGRES_LOCK_TIMEOUT_MILLIS="2000"
export EXAMPLE_SERVICE_POSTGRES_IDLE_TRANSACTION_TIMEOUT_MILLIS="10000"
```

`POSTGRES_TLS_MODE` defaults to `require` and overrides a conflicting
`sslmode` in the connection URI. Plaintext is available only through the exact
value `allow-plaintext-development` for loopback targets. Do not use it in
production. A custom CA and plaintext mode are rejected as contradictory.

The URI remains wrapped in `secrecy::SecretString`, and `Debug` output redacts
both credentials and private CA paths. Session-level statement, lock, and idle
transaction timeouts are bounded and applied to every pooled connection.

## Transactions and retries

Choose transaction isolation and read-only policy explicitly. Use a
transaction-scoped advisory lock for application migrations.
`PostgresQueryErrorReason::retry_hint` classifies failures without retaining
server diagnostics. Before retrying, establish idempotency and set an attempt
limit and deadline.
