# Building and testing

Install the toolchain in `rust-toolchain.toml` and the FoundationDB 7.3.77 C
client library. Contract and Worker checks also require Node 26, pnpm 12.21.0,
Buf 1.75.0, Ripgrep, jq, and these Cargo tools:

```sh
cargo install reallyme-protoc-gen-buffa --version '=0.1.2' --locked
cargo install reallyme-protoc-gen-buffa-packaging --version '=0.1.2' --locked
cargo install connectrpc-codegen --version '=0.9.0' --locked
cargo install worker-build --version '=0.9.0' --locked
rustup target add wasm32-unknown-unknown
```

From the repository root, run:

```sh
scripts/release-readiness/check.sh
```

This runs the shared repository policy, protobuf checks and regeneration,
feature checks, Worker bundle build, release-tool tests, Cargo checks, unit
and documentation tests, Rustdoc, and Clippy. It requires network access for
uncached tools and packages. `cargo deny`, `cargo audit`, and `cargo machete`
run only if installed; the script reports skipped checks.

For individual checks:

```sh
scripts/policy/verify-workspace-standards.sh
scripts/policy/verify-contract-boundaries.sh
scripts/conformance/verify-protobuf-contract.sh
scripts/conformance/verify-example-app-feature-separation.sh
scripts/conformance/verify-server-kit-feature-separation.sh
scripts/build/check-worker.sh
cargo check --locked -p reallyme-platform --no-default-features
```

CI also checks dependencies and licenses, known dependency vulnerabilities,
API compatibility against the latest release tag, each published crate at its
minimum Rust version, and default/no-default feature combinations.

Run live service tests through the manually dispatched
[Live infrastructure tests workflow](../../.github/workflows/live-infrastructure.yml).
Ordinary `cargo test` runs leave these tests ignored. Instructions for Kani
and TLA+ are in [Running formal checks](../../conformance/formal-methods/README.md).

Crates Package Preflight requires the release version and full current `main`
SHA. It verifies Required CI, checks crate archives and dependency order, and
uploads the archives with an attestation. Use its summary command to dispatch
publication for the verified version and SHA.
