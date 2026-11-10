# Validating Platform Changes

Run the repository release-readiness entrypoint before submitting a change:

```console
scripts/release-readiness/check.sh
```

That command runs the shared ReallyMe repository gate, contract generation,
repository policy, feature separation, Rust formatting, workspace checks,
tests, documentation tests, Clippy, Rustdoc, and release-tool tests. It also
checks the facade without default features and builds the Worker bundle.

Install the pinned Rust toolchain from `rust-toolchain.toml`, Node 26, pnpm
12.18.0, Buf 1.75.0, `protoc-gen-buffa` and `protoc-gen-buffa-packaging` 0.9.2,
`connectrpc-codegen` 0.9.0, and `worker-build` 0.8.7. The FoundationDB crate
requires the FoundationDB 7.3.77 C client library. Repository policy checks
require Ripgrep. The gate downloads the pinned shared release-readiness package
and Worker tooling, so a cold environment requires network access.

The script runs `cargo deny`, `cargo audit`, and `cargo machete` only when those
tools are installed and reports each skipped check. CI additionally requires
dependency/license checks, vulnerability checks, published-API compatibility
against the latest release tag, the minimum-supported Rust toolchain check,
and default/no-default feature test runs. The manually dispatched full CI lane
starts disposable services and selects the ignored live infrastructure tests;
the local script does not run those service tests. Package preflight also
inspects crate archives and validates release-source identity.

For a faster development loop, select the narrowest applicable gate:

```console
scripts/policy/verify-workspace-standards.sh
scripts/policy/verify-contract-boundaries.sh
scripts/conformance/verify-example-app-feature-separation.sh
scripts/conformance/verify-server-kit-feature-separation.sh
scripts/build/check-worker.sh
```

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
