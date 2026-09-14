# Example Server

The example server is a complete native host for the reference application. It
compiles one application into one executable, owns the process boundary, and
delegates lifecycle management to `reallyme-server-kit`.

The application owns behavior. The server owns execution.

## Run

From the Platform repository root:

```console
cargo run -p example-server -- \
  --config servers/example/config/example-server.jsonc
```

The server listens on `127.0.0.1:8080`. Call the application and the
runtime-owned operational endpoints from another terminal:

```console
curl http://127.0.0.1:8080/hello
curl http://127.0.0.1:8080/healthz
curl http://127.0.0.1:8080/readyz
curl http://127.0.0.1:8080/version
curl http://127.0.0.1:8080/metrics
```

The application response is:

```json
{"message":"hello from example-app"}
```

Press `Ctrl-C` to exercise coordinated shutdown.

## Composition

The server contains a small amount of deliberate code:

- [`src/composition.rs`](src/composition.rs) builds the listener and native
  runtime.
- [`src/registry.rs`](src/registry.rs) declares the applications compiled into
  the executable.
- [`config/example-server.jsonc`](config/example-server.jsonc) defines
  listener, observability, shutdown, and application-profile policy.
- [`../../apps/example/config/local.jsonc`](../../apps/example/config/local.jsonc)
  defines application-owned behavior and adapter configuration.

The server requires an explicit configuration path. A deployed host can mount
that document from its infrastructure repository or construct the same typed
configuration from an approved configuration service before building the
runtime.

To add an application, add its crate as a dependency and register its native
adapter in `src/registry.rs`. Configuration can select and configure code that
was compiled into the executable; it cannot load arbitrary application code at
runtime.

## Container

[`deploy/Dockerfile`](deploy/Dockerfile) builds the same executable into a
minimal container image. See [`deploy/README.md`](deploy/README.md) for the
local build and run commands.

## Verify

```console
cargo fmt --all -- --check
cargo test -p example-server
cargo clippy -p example-server --all-targets --all-features -- -D warnings
```
