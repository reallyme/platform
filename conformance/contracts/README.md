# Contract Conformance

Contract conformance covers protobuf ownership, generated-source placement,
stable service metadata, and dependency direction. The reference evidence is:

- schemas and generation configuration in `apps/example/contract`;
- deterministic generation through `scripts/generation/generate-rust-contracts.sh`;
- boundary enforcement through `scripts/policy/verify-contract-boundaries.sh`;
- contract and adapter tests in `apps/example`.

Generated wire shapes are never accepted as validated domain values.
