# example-app

`example-app` is the runnable example app used by local development,
CI, and app-template validation.

It is intentionally built in the same shape product apps should use. This crate
is not a toy smoke test; it is the reference implementation for how a
host-neutral app plugs into the ReallyMe platform.

- `contract/` owns raw protobuf, generated Connect/protobuf code, DTOs, typed
  port traits, and contract errors.
- `config/` owns app JSONC behavior settings.
- `app/` owns host-neutral config, context, descriptor, errors, health, and use-cases.
- `ports/` owns concrete downstream port declarations for this app.
- `adapters/connect/` owns generated Connect RPC service wiring.
- `adapters/http/` owns Axum compatibility routes.
- `adapters/server/` bridges the app into `reallyme-server-kit`.

The reference Worker calls the same host-neutral app use-case from its own
Cloudflare adapter. The app does not need a Cloudflare feature or a dependency
on `workers-rs` to run at the edge.

The app does not start listeners, initialize tracing, manage process shutdown,
own process readiness, or supervise long-running tasks. Those concerns stay in
host/runtime kits.

## Vocabulary

- `app`: business/use-case unit that owns behavior and app contracts.
- `protobuf service`: generated RPC trait declared in `.proto`; an app may
  expose one or more of these.
- `server`: runnable process host that composes apps.
- `server-kit`: reusable runtime/lifecycle framework that defines how servers
  run.
- `app-kit`: reusable host-neutral app contracts/conventions.

This app exposes one protobuf service, `ExampleService`. That service is not a
process, listener, runtime, container, or deployment unit.

## Feature Gates

Adapters are feature-gated so product apps can compile in different host
profiles without dragging in native-only dependencies by accident.

Current features:

- `connect`: enables generated Buffa/Connect RPC types and `connectrpc` router
  wiring.
- `connect-axum`: enables Connect RPC plus the Axum mounting helpers used by
  the native HTTP host.
- `native-server`: enables Axum compatibility routes plus the
  `reallyme-server-kit` adapter. It does not require `connect`; an HTTP-only
  native server app is a valid deployment shape.
- `websocket`: enables the example app WebSocket echo adapter through
  `reallyme-server-kit` WebSocket primitives. It is intentionally separate from
  `native-server` so HTTP-only native apps can stay WebSocket-free.
- `default`: enables `connect-axum`, `native-server`, and `websocket` for native
  development and CI. Standalone tonic gRPC is intentionally not part of this
  example app scaffold.

Useful checks:

```text
cargo check -p example-app
cargo check -p example-app --no-default-features
cargo check -p example-app --no-default-features --features connect
cargo check -p example-app --no-default-features --features native-server
cargo check -p example-app --no-default-features --features native-server,websocket
cargo check -p example-app --no-default-features --features native-server,connect-axum
```

Rules:

- App core code must compile without `server`.
- HTTP/native-server adapters must compile without Connect or tonic gRPC when a
  deployment intentionally exposes only HTTP compatibility routes.
- Worker hosts must call host-neutral app behavior and keep `workers-rs` out of
  the application crate.
- Native server adapters may depend on `reallyme-server-kit`, but only behind
  the `native-server` feature and only inside `adapters/server/`.
- Axum-specific code belongs in `adapters/http/` and is also gated by
  `native-server`, not in `app/`.
- Connect generated transport types stay in contract/adapters and must not leak
  deep into host-neutral app use-cases. If a product app later adds standalone
  tonic gRPC, it should be an explicit `tonic-grpc` feature and adapter, not
  default app scaffolding.

Use direct Cargo feature checks to prove the feature matrix for
`example-app`.

## App Config

App behavior config is JSONC and lives in:

```text
config/local.jsonc
config/staging.jsonc
config/prod.jsonc
```

The app-kit standard envelope owns common fields such as:

- `public_base_url`
- `cors.allowed_origins`
- `timeouts.request_timeout_millis`
- `limits.request_body_limit_bytes`
- `limits.max_inflight_requests`
- `reflection_enabled`
- `downstream.<port_name>.base_url`

Apps may add typed custom config fields, but they should not reimplement the
standard envelope. Server composition chooses which app config profile to load
and injects the validated config into app context. App core code must not read
local files or environment variables directly.

`reflection_enabled` is present only as a reserved, reviewed policy field. It
must remain `false`; app-kit rejects `true` so example/template-derived apps do
not accidentally expose schema introspection.

## How Requests Flow

The example `Hello` flow is deliberately small:

```text
Connect RPC /reallyme.example.v1.ExampleService/Hello
  -> adapters/connect::ExampleConnectService
  -> app::hello(...)
  -> app core response
  -> generated protobuf response

HTTP compatibility /hello
  -> adapters/http route
  -> app::hello(...)
  -> HTTP response

reallyme-server host
  -> adapters/server::runtime_app()
  -> server-kit mounts app router under server JSONC mount
  -> server-kit adds /healthz, /readyz, /version, /metrics

Cloudflare Worker /hello or Connect RPC path
  -> workers/example fetch entrypoint
  -> Worker-host route adapter
  -> app::hello(...)
  -> Worker response
```

The important rule is that adapters call the same app-layer use-case. Do not
fork behavior between Connect, gRPC, HTTP, server, and Workers adapters.

## RPC Generation

`contract/proto/` is the canonical contract for RPC-shaped behavior. This app
follows the Connect Rust production workflow from inside the contract crate:

```text
cd apps/example/contract
buf lint
buf generate
```

`buf generate` is expected to use `protoc-gen-buffa`,
`protoc-gen-connect-rust`, and `protoc-gen-buffa-packaging` as configured in
`contract/buf.gen.yaml`. Generated Rust is committed under
`contract/src/generated/`. App adapters import generated code from
`reallyme-example-contract`; they do not own a second generated tree. Do not
add Cargo `build.rs` protobuf generation for this app.

The protobuf `service ExampleService` declaration is one RPC service contained
by the example app. It is not a server process and must not own listeners,
shutdown, readiness, tracing initialization, or runtime lifecycle.

## Adding a New RPC Service

Use this sequence for new app-owned protobuf/RPC services.

1. Add or update the `.proto` file under
   `contract/proto/reallyme/example/v1/`.
2. Define request/response messages with stable field names and comments.
3. Add the protobuf `service` and RPC methods.
4. Run `buf lint`.
5. Run `buf generate`.
6. Commit generated Buffa/Connect code under `contract/src/generated/`.
7. Add or update host-neutral DTOs/ports/errors in `contract/src/`.
8. Add a host-neutral request/response/use-case in `src/app/`.
9. Implement the generated Connect trait in `src/adapters/connect/`.
10. Add or update canonical path constants in `src/adapters/connect/path.rs`.
11. Mount the Connect route in `src/adapters/http/router.rs` when the native
    HTTP host should expose it.
12. Add gRPC/Workers adapters only if the app actually needs those host
    surfaces.
13. Add tests for the app use-case and each adapter that exposes it.

Adapter tests should prove:

- generated Connect route calls the host-neutral app core
- public error mapping does not leak internals
- HTTP compatibility route, if any, calls the same app use-case
- generated route constants match protobuf/gRPC service names
- feature-gated builds still compile

Do not:

- add protobuf generation to Cargo `build.rs`
- put business logic in generated-code adapters
- let generated protobuf types become the app core model
- bypass `reallyme-server-kit` standard HTTP layers in native server hosts
- introduce app-owned listener/shutdown/readiness code

## Adding App Behavior

For new behavior, add code in this order:

1. Define host-neutral request/response/error types in `src/app/`.
2. Add or extend typed app config in `src/app/config.rs` if behavior needs
   configuration.
3. Add app-specific downstream ports in `src/ports/` if behavior calls another
   app or external system.
4. Wire the behavior into `ExampleAppContext` only through typed constructors.
5. Add transport adapters under `src/adapters/`.
6. Add server composition only in the server crate/config, not in app core.

Keep app behavior deterministic and fail-closed when a downstream port is
unconfigured.

## Running Through the Example Server

The local server composition mounts this app:

```text
cargo run -p example-server -- \
  --config servers/example/config/example-server.jsonc
```

Server observability config supports request-completion logging control through
`observability.request_log_mode` in the server JSONC:

- `disabled`: do not emit per-request completion logs
- `errors_only`: emit only for error responses
- `sampled`: emit sampled success logs plus all error/slow requests
- `all`: emit every request completion (default local/dev behavior)

Optional tuning fields:

- `observability.request_log_sample_rate`: `0.0..=1.0` (used by `sampled`)
- `observability.request_log_slow_request_ms`: always log requests at/above this latency

Expected local checks:

```text
curl http://127.0.0.1:8080/hello
curl http://127.0.0.1:8080/healthz
curl http://127.0.0.1:8080/readyz
curl http://127.0.0.1:8080/version
curl http://127.0.0.1:8080/metrics
buf curl --schema apps/example/contract/proto --protocol connect --data '{}' \
  http://127.0.0.1:8080/reallyme.example.v1.ExampleService/Hello
```

The default example build also exposes `GET /ws` as a minimal bounded
WebSocket echo route. It exists to prove the native server WebSocket foundation;
real product apps should define their own protocol messages below app core and
keep payload logging disabled.

If a port is already in use, change the server JSONC composition rather than
hardcoding listener behavior in this app.

## Running Through Cloudflare Workers

The concrete Worker host lives in:

```text
workers/example/
```

It imports this app with `default-features = false` and `features = ["connect"]`,
proving that Worker builds do not require Axum, `reallyme-server-kit`, or a
Cloudflare dependency in the app crate.

Run locally:

```text
cd workers/example
rustup target add wasm32-unknown-unknown
cargo install worker-build
wrangler dev --config wrangler.jsonc
```

Expected local checks:

```text
curl http://127.0.0.1:8787/hello
curl http://127.0.0.1:8787/healthz
curl http://127.0.0.1:8787/readyz
curl http://127.0.0.1:8787/version
curl http://127.0.0.1:8787/metrics
buf curl --schema ../../apps/example/contract/proto --protocol connect --data '{}' \
  http://127.0.0.1:8787/reallyme.example.v1.ExampleService/Hello
```

The Worker `/metrics` route is intentionally a compatibility endpoint. Native
server processes expose Prometheus through `reallyme-server-kit`; Workers
should use Cloudflare Workers observability for platform metrics.

## Validation

Before using this app as a reference for a new app, run:

```text
cd apps/example/contract
buf lint
buf generate
cd ../../..
cargo fmt --all
cargo check
cargo test
cargo test --doc
cargo clippy --all-targets --all-features -- -D warnings
```
