# reallyme-foundationdb-kit

FoundationDB connections, tenant handles, transactions, and operator commands.
Requires the FoundationDB 7.3 C client; CI uses 7.3.77 with API version `730`.
FoundationDB 7.3 and foundationdb-rs label tenant support experimental. Test
upgrades, restores, and rollback procedures before using tenants in production.

## Connect and configure

Construct one `FoundationDbConnector` per process and share it through clones.
`connect` creates a lazy database handle. Call `connect_and_check`,
`health_check`, or `verify_ready` before accepting traffic. `verify_ready`
also checks that required tenants exist and have compatible metadata;
application startup never creates tenants.

`connect` and `connect_and_check` are unsafe: drop all tenant handles and
connector clones before normal process exit. Do not call `std::process::exit`
while they are live. The FoundationDB client cannot restart after shutdown.

`FdbConfig::from_env_prefix("EXAMPLE")` reads:

| Variable | Value |
| --- | --- |
| `EXAMPLE_FDB_CLUSTER_FILE` | Optional readable cluster file; otherwise use the client default. |
| `EXAMPLE_FDB_API_VERSION` | `730`; other versions are rejected. |
| `EXAMPLE_FDB_HEALTH_CHECK_TIMEOUT_MILLIS` | Default `5000`, maximum `60000`. |

Without a prefix, the loader uses `FDB_*`. Prefixes accept uppercase ASCII
letters, digits, and underscores. Cluster-file paths are redacted in diagnostics.

## Transactions

Open a provisioned tenant with `open_tenant`. Callbacks receive a
`TenantDataTransaction` and use `TenantDataKey` or `TenantDataRange`; application
keys cannot address system keys or the reserved `__meta` namespace.
Versionstamped-key atomic mutations are unsupported.

Use `ReadTxnPolicy`/`idempotent_read_option` for reads and
`WriteTxnPolicy`/`mutation_option` for writes. Read-policy transactions reject
mutations, including when the callback ignores the method's error.

Retry limits include the initial attempt. Callbacks may run more than once.
The C API timeout bounds database operations; the adapter checks its budget
before callbacks and commit submission. Callbacks must enforce cancellation
and deadlines for their own asynchronous work. An already submitted commit
can finish after a timeout, leaving an uncertain outcome. Non-idempotent writes
do not automatically retry `maybe_committed`; callers must resolve that outcome
using their operation IDs or reconciliation procedure.

## Tenant administration

Build the operator binary separately from application binaries:

```sh
cargo build -p reallyme-foundationdb-kit --bin fdb-tenant-admin --features tenant-admin
```

Run `target/debug/fdb-tenant-admin`, or put the binary on `PATH` for the commands
below. Set `FDB_*` for the intended cluster. With `tenant-admin`, use a native client library compatible
with that cluster; the operator does not use multiversion client switching.

```sh
fdb-tenant-admin ensure example
fdb-tenant-admin exists example
fdb-tenant-admin id example
```

`ensure` provisions a tenant and metadata. An existing tenant with missing or
incompatible metadata is rejected. For interrupted provisioning, `repair example`
initializes metadata only when the tenant is empty.

### Delete and resume

Record the output of `id example` before deletion. Deletion refuses application
data. Clearing kit metadata and removing the tenant are separate commits;
interruption can leave a surviving tenant without usable metadata. Use a durable
record for recovery:

```sh
fdb-tenant-admin delete example --record /var/lib/fdb-operator/example-delete.intent
fdb-tenant-admin resume-delete /var/lib/fdb-operator/example-delete.intent
```

Use a new record path in an existing operator-owned directory on a local
filesystem supporting file locks, atomic rename, and directory sync. The `.lock`
extension is reserved. Retain the record and sibling lock file until
reconciliation completes. Resume an interrupted deletion before starting another
lifecycle operation for that tenant.

`resume-delete` rejects a different cluster UID. It returns `original-absent`
when the recorded tenant ID no longer owns the name, or `metadata-safe` after
repairing the surviving original. Partial or incompatible metadata and
unavailable lookups leave the record pending for review or retry. Completed
records replay their saved outcome.

For a deletion without a durable record, `recover-delete <tenant> <expected-tenant-id>`
requires a trusted ID recorded before deletion. It recreates metadata only when
the reserved namespace is empty, permits existing application data, and records
a new metadata creation time. It rejects a replacement tenant with another ID.

### Coordinate operators across hosts

Use one dedicated control tenant on the same standalone cluster for all
cooperating operators:

```sh
fdb-tenant-admin ensure operator-control
export FDB_TENANT_ADMIN_CONTROL_TENANT=operator-control
fdb-tenant-admin delete example --record /var/lib/fdb-operator/example-delete.intent
fdb-tenant-admin resume-delete /var/lib/fdb-operator/example-delete.intent
# Another host can recover an expired operation without its local record:
fdb-tenant-admin takeover-delete example
```

With the control tenant configured, `delete` requires `--record`. Resume pending
legacy records with the setting unset before enabling coordination. New records
retain the control tenant and operation IDs and can resume without the setting.

Resume a failed operation or take it over after reservation expiry. The lease
expires after 15 million read versions, about 15 seconds under normal version
advancement. Takeover reconciles the original tenant ID; it does not start a new
delete against a replacement. Keep the control tenant and reservation records
available to operators. Coordination applies only to commands using that control
tenant.

### Upgrade record readers

Version 0.4.0 reads historical binary records and writes protobuf at the existing
metadata and control keys. The CLI reads all three historical text-intent formats
and upgrades them when recording an outcome. Snapshot restoration preserves the
saved bytes.

Upgrade application readers and operator binaries together before allowing new
writes on a shared cluster. Older binaries cannot read protobuf records or
intent files.

### Compact and reclaim completed records

After upgrading every operator sharing the control tenant, compact a completed
reservation:

```sh
fdb-tenant-admin prune-delete /var/lib/fdb-operator/example-delete.intent
```

This retains a tombstone with the tenant ID and outcome. Pending local records
can replay a cluster completion; active or missing reservations cannot be
pruned. Keep tombstones until the admission cutoff excludes their generation.

Upgrade every writer before advancing the cutoff. Historical records without a
generation use zero:

```sh
fdb-tenant-admin delete-cutoff example
fdb-tenant-admin advance-delete-cutoff example 1
```

Advancement requires an empty active slot. If the reply is lost, retry the same
target generation. Older pending files can replay retained completions; without
a cluster record they report expiry.

```sh
fdb-tenant-admin reclaim-delete /var/lib/fdb-operator/example-delete.intent
```

Reclamation removes an eligible tombstone and returns `retired`. Retrying the
same record is supported. Completed local files keep their outcome, but pending
files whose tombstones were reclaimed report expiry. Save any required operator
outcome before reclamation.

## Metrics and tests

Enable `metrics` for health counts/latency, transaction attempts/retries/conflicts,
commit latency, and tenant-open failures. Use fixed declared tenant names;
user, request, and key identifiers must not become labels.

Unit tests run without a cluster. Run cluster tests in separate processes or
containers because the FoundationDB client cannot restart in one process.
See [Building and testing](../../docs/guides/validation.md) for the workspace and
live-service checks.
