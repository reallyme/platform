# Feature-Matrix Conformance

The feature matrix proves that optional transports and hosts do not leak into
the host-neutral core. Run:

```console
scripts/conformance/verify-example-app-feature-separation.sh
scripts/conformance/verify-server-kit-feature-separation.sh
cargo check -p reallyme-platform --no-default-features
```

The Platform facade has an empty default feature set. Runtime and
infrastructure integrations remain opt-in.
