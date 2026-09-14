# reallyme-postgres-kit

Shared PostgreSQL configuration and pooling primitives for ReallyMe services.

The kit is the first-class PostgreSQL infrastructure adapter for native
ReallyMe platform apps. It owns:

- Redacted PostgreSQL connection configuration.
- Environment-prefix loading for app hosts.
- Certificate-validated Rustls transport, required by default.
- `bb8`/`tokio-postgres` pool construction and bounded server-side timeouts.
- Readiness reports with pool occupancy and active TLS state.
- Explicit transaction isolation/read-only policies.
- Typed begin/commit/rollback helpers that preserve retry-relevant failure
  classes without retaining server diagnostics.
- Transaction-scoped advisory locking for app-owned schema migrations.
- Typed, low-cardinality errors.
- Stable SQLSTATE classification and conservative retry hints; raw PostgreSQL
  error messages never cross the kit boundary.

## Use from a native adapter

Application core crates should expose repository ports and remain independent
of PostgreSQL. A native host or infrastructure adapter owns the pool:

```rust,no_run
use reallyme_postgres_kit::{PostgresConfig, PostgresPool, PostgresResult};

async fn connect_postgres() -> PostgresResult<PostgresPool> {
    let config = PostgresConfig::from_env_prefix("EXAMPLE_SERVICE")?;
    let pool = PostgresPool::connect(&config).await?;
    pool.health_check().await?;
    Ok(pool)
}
```

`PostgresPool::connect` eagerly establishes the configured minimum connection
count (one by default), so successful construction is a real reachability and
TLS/authentication gate. Setting the minimum to zero is supported for explicit
lazy test fixtures, but is not the production default.

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
value `allow-plaintext-development`; production configuration must never use
that mode. A custom CA and plaintext mode are rejected as contradictory.

The URI remains wrapped in `secrecy::SecretString`, and `Debug` output redacts
both credentials and private CA paths. Session-level statement, lock, and idle
transaction timeouts are bounded and applied to every pooled connection.

## Ownership boundary

App crates keep schema and repository semantics in app-owned adapters while
using this kit for connection lifecycle, transaction policy, migration
coordination, and readiness. FoundationDB and PostgreSQL remain peer storage
adapters behind app-owned repository ports; this kit does not invent a false
common transaction model for databases with different semantics.

Apps should translate `PostgresError` into their own port error at the adapter
boundary. `PostgresQueryErrorReason::retry_hint` is intentionally conservative:
it never removes the caller's obligation to prove idempotency and enforce a
bounded retry/deadline policy.
