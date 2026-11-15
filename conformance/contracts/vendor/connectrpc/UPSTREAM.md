# ConnectRPC for the Platform examples

This private crate comes from the [ConnectRPC 0.9.1 registry archive][archive]
at upstream commit [`7fd4a8248df90a6f5403c7066df0e44d4164d67f`][source].
The archive SHA-256 is
`457cfa540c62e1b0b05db676cc9a9129162ad94d3a3eec6f2d19706c53acae7d`.
The upstream Apache-2.0 license is in [LICENSE](LICENSE).

The example transport and generated contracts must use the same Buffa
`Message` and view traits. This copy aliases `reallyme-buffa` and its test
types at exactly 0.1.2. Constructors and dispatch paths that create
`OwnedView` require `ViewLifetimeParametric`. Accessors that convert to
owned messages require `ViewReborrow`. These bounds let Buffa enforce the
lifetime of views backed by an owned buffer. The patch adds no unsafe
implementations and keeps the upstream message limits, codecs, and routing.
Two lint changes keep the standalone checks clean on Platform's toolchain.
Stream termination keeps its record inline to avoid allocating on normal
completion, and the HTTP/2 test uses fixed-size settings entries.
The idle-reaping test pauses Tokio after its first loopback response so
OS socket readiness cannot advance the idle timer during setup.

The separate `protoc-gen-connect-rust` plugin emits service stubs that
reference the generated example messages. It runs as a separate executable,
so its own dependencies do not enter the example's runtime graph.
Platform's public crates do not depend on this copy.

The Rust source under `src/generated` comes from the registry archive plus
[`buffa-compatibility.patch`](buffa-compatibility.patch). Change the patch
and regenerate the source together. From the Platform root, verify the
checked-in source and run the transport suite against its own lockfile:

```sh
scripts/generation/import-connectrpc-source.sh --check /path/to/connectrpc-0.9.1.crate
cargo test --manifest-path conformance/contracts/vendor/connectrpc/Cargo.toml --locked --all-features --lib
```

Use `--write` to regenerate the source from the verified archive. When
adopting a new upstream transport, record the new archive checksum and
commit, reapply or retire the patch, and run the native and Worker example
tests along with the transport suite.

[archive]: https://crates.io/crates/connectrpc/0.9.1
[source]: https://github.com/connectrpc/connect-rust/tree/7fd4a8248df90a6f5403c7066df0e44d4164d67f/connectrpc
