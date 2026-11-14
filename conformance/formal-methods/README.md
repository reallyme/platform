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
temporary copy of the harness and production module so the verifier can use its
own compiler without building the server kit or writing into the source tree.

## Listener route policy

[The Kani harness](policy_path.rs) checks the production escape decoder used
by the [path canonicalizer](../../kits/server/src/http/layers/route_visibility/policy_path.rs).
It checks all 65,536 pairs of escape bytes against the decoder's acceptance
policy, including mixed-case hex digits, malformed escapes, and decoded bytes
that would introduce another escape, a separator, or NUL.
[Middleware tests](../../kits/server/src/http/layers/route_visibility/path_canonicalization_tests.rs)
cover whole paths, including malformed escapes and multibyte UTF-8.
The [route visibility tests](../../kits/server/src/http/layers/route_visibility/encoded_carve_out_tests.rs)
exercise agreement between the raw and canonical route rules.

## FoundationDB tenant deletion and recovery

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
a single deletion and operator recovery with a matching recorded ID. Further
verification needs to track late commits, tenant name reuse, stale handles
across recreation, metadata values, and retry progress.
Use the FoundationDB integration suite to validate the production adapter
against the database.
