# Package Conformance

Package conformance ensures published crates contain only intentional source,
documentation, and metadata, and that their dependency order is reproducible.

The release lane uses `cargo package --list`,
`scripts/release/compare_crate_payloads.mjs`, and
`scripts/release/verify_release_source.mjs`. Public manifests use explicit
`include` allowlists and publish only to crates.io; all reference apps and hosts
remain private.
