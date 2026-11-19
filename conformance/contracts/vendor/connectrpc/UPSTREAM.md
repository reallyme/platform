# Vendored ConnectRPC

This private example dependency is based on the [ConnectRPC 0.9.1 registry
archive][archive] at upstream commit
[`7fd4a8248df90a6f5403c7066df0e44d4164d67f`][source]. Archive SHA-256:
`457cfa540c62e1b0b05db676cc9a9129162ad94d3a3eec6f2d19706c53acae7d`.
The upstream [Apache-2.0 license](LICENSE) and [README](README.md) are retained.

Local changes are in [buffa-compatibility.patch](buffa-compatibility.patch).
The transport uses ReallyMe Buffa and Buffa Types 0.1.2; generated example
messages must use that runtime too. Public Platform crates do not depend on
this vendored copy. The Connect generator runs separately.

From the Platform root, reproduce and test the patched source:

```sh
scripts/generation/import-connectrpc-source.sh --check /path/to/connectrpc-0.9.1.crate
cargo test --manifest-path conformance/contracts/vendor/connectrpc/Cargo.toml \
  --locked --all-features --lib
```

Edit the patch rather than `src/generated`. Use `--write` with the verified
archive to regenerate that directory. For an upstream update, update the archive
checksum and commit, reapply the patch, and run the transport suite and native
and Worker example tests.

[archive]: https://crates.io/crates/connectrpc/0.9.1
[source]: https://github.com/connectrpc/connect-rust/tree/7fd4a8248df90a6f5403c7066df0e44d4164d67f/connectrpc
