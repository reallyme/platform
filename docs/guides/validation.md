# Validating Platform Changes

Run the repository release-readiness entrypoint before submitting a change:

```console
scripts/release-readiness/check.sh
```

That command runs the shared ReallyMe repository gate, Buf lint and breaking
checks, contract generation and coverage checks, repository policy, feature
separation, Rust formatting, workspace checks, tests, documentation tests,
Clippy, Rustdoc, and release-tool tests. It also checks the facade without
default features and builds the Worker bundle.

Install the pinned Rust toolchain from `rust-toolchain.toml`, Node 26, pnpm
12.21.0, Buf 1.75.0, the `reallyme-protoc-gen-buffa` and
`reallyme-protoc-gen-buffa-packaging` packages at exactly 0.1.2,
`connectrpc-codegen` 0.9.0, and `worker-build` 0.9.0. The FoundationDB crate
requires the FoundationDB 7.3.77 C client library. Repository checks require
Ripgrep and jq. The gate downloads the pinned shared release-readiness
package and Worker tooling, so a cold environment requires network access.

The script runs `cargo deny`, `cargo audit`, and `cargo machete` only when those
tools are installed and reports each skipped check. CI additionally requires
locked dependency and license checks, vulnerability checks, published-API
compatibility against the latest release tag, all-feature tests at each
published crate's declared minimum Rust version, and default/no-default feature
test runs. The manually dispatched Live infrastructure tests workflow starts
disposable services and runs the ignored service tests.

Crates Package Preflight requires the release version and full current
`main` SHA. It verifies Required CI for that SHA, inspects the crate archives,
and uploads them with the release attestation. Its summary includes a command
to dispatch publication for the verified version and SHA.

For a faster development loop, select the narrowest applicable gate:

```console
scripts/policy/verify-workspace-standards.sh
scripts/policy/verify-contract-boundaries.sh
scripts/conformance/verify-protobuf-contract.sh
scripts/conformance/verify-example-app-feature-separation.sh
scripts/conformance/verify-server-kit-feature-separation.sh
scripts/build/check-worker.sh
```

The formal verification lane runs Kani proofs and TLA+ model checks against
the same verified commit. See the [formal verification checks](../../conformance/formal-methods/README.md).

The native and Worker adapters share the
[host-parity fixture](../../conformance/host-parity/cases.json) for API success
and failure cases. Package preflight inspects each crate's explicit source
allowlist and verifies workspace dependency order.

The workspace-standards gate enforces inherited lints, package visibility,
SPDX headers, generated-code placement, the ban on wildcard Rust imports, and a
hard 500-line ceiling for hand-written production Rust files. Test-only modules
retain their separate 800-line ceiling.

The facade must always be checked without default features because adding a
runtime or infrastructure dependency to its default graph is a compatibility
and portability regression:

```console
cargo check -p reallyme-platform --no-default-features
```
