# Host-Parity Conformance

Host parity means the same application core and contract semantics execute in
the native reference server and the reference Worker. Host adapters may differ
in lifecycle and transport integration, but must not fork business behavior.

Evidence lives in `servers/example/tests`, `workers/example/src/tests.rs`, and
the host-neutral tests under `apps/example`. The native and Worker adapter
tests both read `cases.json` for shared success and failure expectations. CI
builds and tests both hosts.
