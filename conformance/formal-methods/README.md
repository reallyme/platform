# Running formal checks

The [workflow](../../.github/workflows/formal-methods.yml) uses Kani 0.68.0,
Java 21, and the TLA+ 1.7.4 tools JAR.

```sh
cargo +stable install --locked kani-verifier --version 0.68.0
cargo +stable kani setup
TLA2TOOLS_JAR=/path/to/tla2tools.jar bash scripts/conformance/verify-formal-methods.sh
```

Download the JAR from the
[TLA+ 1.7.4 release](https://github.com/tlaplus/tlaplus/releases/tag/v1.7.4)
and verify its SHA-256 against the workflow. Kani installs its own Rust
compiler; the runner builds harnesses in temporary storage.

Pass `kani` or `tlc` to the script to run one group:

```sh
bash scripts/conformance/verify-formal-methods.sh kani
TLA2TOOLS_JAR=/path/to/tla2tools.jar bash scripts/conformance/verify-formal-methods.sh tlc
```

The Rust harnesses check route admission, proxy trust, gRPC authorization and
request deadlines, tenant key ranges, retry budgets, and rate-limit arithmetic.
The TLA+ models cover server lifecycle and FoundationDB deletion, recovery,
reservation, and record reclamation. Model assumptions and bounds are declared
in the `.tla` and `.cfg` files.

Some configurations violate an invariant to test that the checker
finds the expected counterexample. The runner checks that outcome; a different
failure, cancellation, or missing counterexample fails the run. Use the runner's
exit status rather than interpreting every TLC invariant failure as a failed
suite.

These checks cover the configured finite models and harness inputs. They do not
verify database or network implementations or replace the live infrastructure
and transport tests. Run those through the
[validation procedures](../../docs/guides/validation.md).
