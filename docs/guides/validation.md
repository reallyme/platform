# Validating Platform Changes

Run the repository release-readiness entrypoint before submitting a change:

```console
scripts/release-readiness/check.sh
```

That command runs the shared ReallyMe repository gate, contract generation,
repository policy, feature separation, Rust formatting, workspace checks,
tests, documentation tests, Clippy, and release-tool tests. It is the local
equivalent of the required CI lanes.

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
