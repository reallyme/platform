# example-app

The reference application exposes a `Hello` RPC, `GET /hello`, and an optional
WebSocket echo route. The [native server](../../servers/example/) and
[Worker](../../workers/example/) run the same application core.

## Run

From the repository root:

```sh
cargo run -p example-server -- \
  --config servers/example/config/example-server.jsonc
```

```sh
curl http://127.0.0.1:8080/hello
buf curl --schema apps/example/contract/proto --protocol connect --data '{}' \
  http://127.0.0.1:8080/reallyme.example.v1.ExampleService/Hello
```

The default native build also exposes `GET /ws`. Standalone Tonic gRPC is not
implemented by this application. For Cloudflare setup and authenticated probes,
use the [Worker instructions](../../workers/example/README.md).

## Configuration and features

`config/local.jsonc`, `config/staging.jsonc`, and `config/prod.jsonc` provide
application profiles. `hello_enabled` controls the greeting. Standard fields
are described in [app-kit](../../kits/app/README.md). The native host chooses
the profile with `application_profile`; listener addresses, timeouts, body limits,
and logging are host settings. Its JSONC parser does not accept an
`observability` object.

| Feature | Enables |
| --- | --- |
| `contract` | Generated protobuf DTOs. |
| `connect` | Contract DTOs and Connect router. |
| `connect-axum` | Connect and Axum mounting helpers. |
| `http` | HTTP routes and server-kit HTTP primitives. |
| `native-server` | HTTP and the native runtime adapter. |
| `websocket` | HTTP and the WebSocket echo adapter. |
| `testing` | Local test-context constructor. |

Default features are `connect-axum`, `native-server`, and `websocket`.
Use `default-features = false` for the core or Worker integration. The Worker
selects `contract`; it does not require a `workers` feature in this crate.

## Changing the application

Use `src/app/` for use cases and validation, `src/ports/` for outbound
capabilities, and `src/adapters/` for transport and native-host integration.
The [architecture guide](../../docs/concepts/architecture.md) defines the
crate boundaries.

For an RPC change, update `contract/proto/`, the DTOs and ports in
`contract/src/`, and canonical RPC paths in `contract/src/rpc.rs`. Regenerate
from the repository root:

```sh
(cd apps/example/contract && buf lint)
scripts/generation/generate-rust-contracts.sh
```

Commit generated files under `contract/src/generated/`. Implement the generated
service trait in `src/adapters/connect/`, register it in `router.rs`, and mount
it through `src/adapters/server/runtime.rs`. Keep Worker request conversion in
the Worker crate. Test the use case and each affected adapter.

The transport uses the same ReallyMe Buffa runtime as the generated messages.
See the [ConnectRPC import instructions](../../conformance/contracts/vendor/connectrpc/UPSTREAM.md)
when updating that dependency.

```sh
scripts/conformance/verify-example-app-feature-separation.sh
cargo test -p example-app
```

For workspace checks and generator versions, see
[Building and testing](../../docs/guides/validation.md).
