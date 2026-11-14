# Formal verification

Run the same checks as the [formal methods workflow](../../.github/workflows/formal-methods.yml)
with Kani 0.68.0, a Java 21 JDK, and the TLA+ 1.7.4 tools JAR.

```sh
cargo +stable install --locked kani-verifier --version 0.68.0
cargo +stable kani setup
TLA2TOOLS_JAR=/path/to/tla2tools.jar bash scripts/conformance/verify-formal-methods.sh
```

The [Kani installer](https://model-checking.github.io/kani/install-guide.html)
uses its own Rust toolchain. Download the JAR from the
[TLA+ 1.7.4 release](https://github.com/tlaplus/tlaplus/releases/tag/v1.7.4)
and verify its SHA-256 against the value in the workflow. The runner builds a
temporary copy of the harnesses and production modules so the verifier can use its
own compiler without building the server kit or writing into the source tree.

## Listener route policy

[The Kani harness](policy_path.rs) checks the production escape decoder used
by the [path canonicalizer](../../kits/server/src/http/layers/route_visibility/policy_path.rs).
It checks all 65,536 pairs of escape bytes against the decoder's acceptance
policy, including mixed-case hex digits, malformed escapes, and decoded bytes
that would introduce another escape, a separator, or NUL.
[Middleware tests](../../kits/server/src/http/layers/route_visibility/path_canonicalization_tests.rs)
cover whole paths, including malformed escapes and multibyte UTF-8.

The [route admission harness](route_admission.rs) checks the production
[admission decision](../../kits/server/src/http/visibility/admission.rs).
An explicit rule can admit a request only when both path spellings select
the same rule and both matches permit the listener. The default can admit
only two unmatched paths. The proof quantifies over the classifications,
the default, and rule equality. Rule matching and listener restrictions
are exercised through the [middleware tests](../../kits/server/src/http/layers/route_visibility/encoded_carve_out_tests.rs).

The [route selection harness](route_selection.rs) checks the production
[ranking function](../../kits/server/src/http/visibility/selection.rs) with
zero, one, or two matching candidates. For two candidates it quantifies over
all representable lengths and both match kinds. The longer match wins and
exact wins at equal length. The [policy tests](../../kits/server/src/http/visibility/tests.rs)
exercise segment boundaries and verify that a denied winner blocks admission
even when a less specific rule and the default would both allow the listener.

## FoundationDB tenant deletion

[The TLA+ model](TenantDeletion.tla) follows the
[tenant admin operations](../../kits/foundationdb/src/fdb/tenant/admin.rs).
The model starts with one tenant incarnation and explores both empty and
nonempty tenants, partial metadata, a concurrent application write, an
uncertain commit, and interruption or failure before recovery. It checks that
deletion preserves application data and foreign metadata, a successful delete
requires tenant absence, and explicit recovery initializes both kit metadata
keys only when the reserved namespace is empty.

Each modeled commit is atomic. The model assumes that the non-snapshot range
read conflicts with writes before the metadata clear, and that FoundationDB
checks tenant emptiness at deletion. Each uncertain commit resolves before
the model advances to lookup or restoration. The model represents the
automatic restoration attempts by their final outcome. It checks safety for
a single deletion and operator recovery with a matching recorded ID. The
model abstracts metadata values and retry progress.
Use the FoundationDB integration suite to validate the production adapter
against the database.

## FoundationDB recovery after name reuse

[TenantRecovery.tla](TenantRecovery.tla) starts after an interrupted delete
has cleared the kit metadata. It models one recovery attempt, one tenant
name, and two tenant IDs. The name can disappear and then resolve to the
replacement ID. The model separates the management lookup, opening by name,
and resolving the keyspace for the recovery transaction. It also covers
concurrent population of the reserved metadata namespace and a commit that
may complete after the caller times out.

[The lookup-only configuration](TenantRecoveryLookupOnly.cfg) represents the ID
check in [recover_interrupted_delete](../../kits/foundationdb/src/fdb/tenant/admin.rs).
The lookup can accept the recorded ID and the later name-based open can
resolve to the replacement. If its metadata namespace is empty, recovery
can initialize the replacement. The runner requires TLC to report precisely
that invariant violation.

[The bound-ID configuration](TenantRecovery.cfg) models a prospective
protocol that checks the same resolved ID used by the recovery transaction.
It checks that committed writes target only the recorded tenant, success
requires a commit, and recovery writes both kit keys only when the namespace
is empty. The model assumes that a bound transaction cannot be retargeted,
that deletion invalidates writes to its old ID, and that the non-snapshot
metadata read conflicts with concurrent population. The model omits
cross-cluster identity, retries, and repeated name reuse. The Rust adapter
still uses the lookup-only check.

FoundationDB assigns IDs and keyspaces at tenant creation. In the 7.3.77
client, a name-based tenant object has its own asynchronous ID lookup
and supplies that tenant object to transactions. A binding change should
validate the ID of the same handle used for the write.
[Tenant documentation](https://apple.github.io/foundationdb/tenants.html),
[native client](https://github.com/apple/foundationdb/blob/7.3.77/fdbclient/NativeAPI.actor.cpp#L2735-L2746),
[thread-safe client](https://github.com/apple/foundationdb/blob/7.3.77/fdbclient/ThreadSafeTransaction.cpp#L227-L243)
