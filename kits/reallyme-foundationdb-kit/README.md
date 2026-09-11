# reallyme-foundationdb-kit

Production FoundationDB connector primitives for ReallyMe services.

This kit is the infrastructure boundary for:

- validated client configuration;
- process-scoped C client/network lifecycle;
- bounded cluster readiness probes;
- typed, non-sensitive errors;
- tenant provisioning validation and tenant-scoped handles;
- explicit read/write transaction policies and retry semantics;
- key/range/tuple construction helpers; and
- low-cardinality optional metrics.

Business schemas, graph behavior, transport adapters, server lifecycle, and
application readiness ownership do not belong here. Applications own their
schemas and map their domain concepts to validated FoundationDB tenant names.

## Upstream production constraint

This kit makes the client boundary production-grade, but it cannot promote an
upstream experimental database feature. FoundationDB 7.3 and foundationdb-rs
still label tenant support experimental. ReallyMe currently requires tenants by
architecture, so deploying this path requires an explicit platform risk
acceptance, representative upgrade/restore testing, and a rollback plan. The
kit fails closed on absent or incompatible tenants; it does not make upstream
tenant lifecycle guarantees stronger than FoundationDB provides.

## Production startup

FoundationDB permits one client API and network initialization per process.
Construct one connector, retain it for the full process lifetime, and clone the
connector when multiple adapters need it:

```rust,no_run
use reallyme_foundationdb_kit::{
    FdbConfig, FoundationDbConnector, FoundationDbTenantName, verify_ready,
};

# async fn start() -> Result<(), reallyme_foundationdb_kit::FdbError> {
let config = FdbConfig::from_env_prefix("HANDLE")?;
let connector = FoundationDbConnector::connect(&config)?;
let tenant_name = FoundationDbTenantName::new("handle");
let Ok(tenant_name) = tenant_name else {
    return Ok(());
};
let _health = verify_ready(&connector, &[tenant_name]).await?;

let tenant = connector.open_tenant(tenant_name).await?;
# drop(tenant);
# drop(connector);
# Ok(())
# }
```

`connect` validates and boots the local client but FoundationDB database handles
are lazy. A production process must call `connect_and_check`, `health_check`, or
`verify_ready` before accepting traffic. `verify_ready` additionally proves that
all required tenants exist and have compatible metadata.

The connector is cloneable because its database and network-lifecycle guard are
owned together. Calling `connect` a second time returns a typed setup error
instead of allowing the upstream binding's duplicate-initialization panic to
cross the kit boundary.

## Configuration

The workspace uses foundationdb-rs `0.11.0` with default recipe/UUID features
disabled and the explicit `fdb-7_3`, embedded-header, and tenant features. CI
installs the pinned FoundationDB 7.3.77 client package and verifies its SHA-256
before installation.

The conventional environment loader reads:

- `FDB_CLUSTER_FILE` (optional; the client default is used when absent);
- `FDB_API_VERSION` (default and required value `730`); and
- `FDB_HEALTH_CHECK_TIMEOUT_MILLIS` (default `5000`, maximum `60000`).

`FdbConfig::from_env_prefix("HANDLE")` reads the same settings as
`HANDLE_FDB_*`. Prefixes must be uppercase ASCII plus digits and underscores.
Invalid Unicode is rejected instead of being treated as an absent value.
The API version is intentionally exact: tenant-management behavior is selected
at compile time by foundationdb-rs, so accepting a different runtime API would
make the configured behavior disagree with the generated 7.3 bindings.

Configured cluster-file paths are bounded, must identify a readable regular
file, and are redacted from `Debug` output. Configuration types never retain or
surface cluster-file contents.

## Readiness and health

`health_report` is bounded by the configured deadline and performs two checks:

1. a local no-op proves the FoundationDB client network thread is processing;
2. obtaining a read version proves real cluster reachability without reading
   application keys.

The returned report contains only the selected API version, observed read
version, and optional client main-thread busyness. Health errors distinguish a
local network failure, cluster unavailability, and deadline expiry without
retaining driver diagnostics or topology.

`verify_ready` then opens every declared logical tenant and validates its
versioned metadata. Tenant creation is never implicit on an application path.

## Tenant lifecycle

Tenant names are deployment-owned identifiers. The kit validates a neutral,
bounded `FoundationDbTenantName`; applications own the mapping from product
concepts to those names.

Provisioning is an explicit operator action:

```sh
cargo run -p reallyme-foundationdb-kit \
  --bin fdb-tenant-admin --features tenant-admin -- ensure example
```

Supported actions are `ensure`, `exists`, and `delete`. Existing tenants with
missing or incompatible metadata fail closed rather than being silently
rewritten.

The `tenant-admin` feature should be enabled only for operator tooling. Runtime
code receives `TenantHandle`, whose transaction helpers remain scoped to the
selected tenant.

## Transaction policy

Use `ReadTxnPolicy`/`idempotent_read_option` for read-only operations and
`WriteTxnPolicy`/`mutation_option` for mutations.

- Retry limits cap total attempts, including the initial attempt, matching
  foundationdb-rs `TransactOption` semantics.
- Timeouts are enforced both as a Rust retry deadline and as a FoundationDB C
  API transaction timeout, so a stalled network call cannot bypass the bound.
- Retry limits are also installed in the C API, not only checked between Rust
  retry-loop iterations.
- Non-retryable errors are returned immediately.
- A non-idempotent mutation never retries `maybe_committed`; callers must resolve
  uncertain outcomes with an operation-specific idempotency invariant.
- Closure data may be replayed and must not assume one execution.
- All key lengths and offsets must continue to use checked arithmetic.

The kit does not make an arbitrary business mutation idempotent. Apps own
operation IDs, deduplication records, and reconciliation policy.

## Metrics

Enable the `metrics` feature to record low-cardinality connector telemetry:

- `reallyme_fdb_health_checks_total{result}`;
- `reallyme_fdb_health_check_latency_seconds`;
- transaction attempts and retries by declared tenant and operation class;
- transaction conflicts by declared tenant and operation class;
- commit latency by declared tenant and operation class; and
- tenant-open failures by declared tenant.

No key, user, request, raw error, cluster path, or arbitrary metadata is used as
a label.

## Validation

The crate's unit tests do not initialize the process-global FoundationDB client.
This keeps test ordering deterministic and allows configuration, key, metadata,
and policy behavior to run without a cluster.

Required local validation:

```sh
cargo fmt -p reallyme-foundationdb-kit -- --check
cargo check -p reallyme-foundationdb-kit --all-features
cargo test -p reallyme-foundationdb-kit --all-features
cargo clippy -p reallyme-foundationdb-kit --all-targets --all-features -- -D warnings
```

Cluster integration tests should run in an isolated process/container because
the FoundationDB client cannot be restarted in-process after shutdown.
