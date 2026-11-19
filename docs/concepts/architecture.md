# Application and host boundaries

Use [apps/example](../../apps/example/) for the application structure,
[servers/example](../../servers/example/) for native composition, and
[workers/example](../../workers/example/) for Cloudflare request adaptation.

## Crate responsibilities

- `crates/platform` re-exports app-kit and selected integrations. Its default
  feature set is empty; integrations require explicit features.
- `kits/app` provides host-neutral configuration, metadata, port descriptors,
  lifecycle contributions, permissions, and metrics conventions.
- An application's `contract` crate contains its protobuf schemas, generated
  bindings, callable ports, DTOs, and contract errors.
- Application core code contains use cases and domain validation. It builds
  without Axum, Tonic, Connect, server-kit, or the Workers runtime.
- Transport adapters validate requests and call the core. They import generated
  bindings from the contract crate and do not depend on other transport adapters.
- Native server crates select applications and dependency bindings, load
  configuration, and compose server-kit. Worker crates supply Cloudflare
  entrypoints, bindings, and request/response conversion.
- Infrastructure kits supply service clients and connection policies.
  Application adapters supply schemas, tenant selection, and business rules.

For outbound calls, define a port in the calling application and provide its
implementation from the host. Calls to another application depend on that
application's contract crate, rather than its implementation.

## Contracts

Protocol Buffers define Platform-owned application data contracts, API messages,
and persisted records. Schemas live in `apps/example/contract/proto/` and
`apps/platform/contract/proto/`. Generated Rust belongs under the owning crate's
`src/generated/` directory. Do not edit generated files by hand.

Run `scripts/conformance/verify-protobuf-contract.sh` to lint schemas, check
compatibility against the last release and change base, regenerate bindings,
and compile their owning packages. See the [validation guide](../guides/validation.md)
for tool versions.

Connect RPC is the primary application RPC transport. HTTP/JSON adapters use
ProtoJSON; standalone Tonic gRPC is optional. FoundationDB system records, gRPC
health, Prometheus metrics, and third-party services retain their upstream wire
formats. Operator configuration uses JSONC.

The HTTP compatibility schema preserves lowercase enum literals. Its Buf naming
exceptions apply only to that schema. Validate decoded messages before passing
values to application code; generated types do not enforce domain invariants.

## Host integration

Hosts choose the applications and adapters compiled into the executable.
Configuration can select compiled applications but cannot load new code.
Hosts also resolve secrets and supply validated application configuration.

Server-kit runs native listeners, readiness, managed tasks, tracing, metrics,
and shutdown. Applications contribute checks and tasks through app-kit
contracts. HTTP listeners can respond before startup checks finish; traffic
routers must honor `/readyz`. The runtime withdraws readiness before draining.

Worker hosts use Cloudflare's lifecycle and call the same application core.
Cloudflare types and bindings remain in the Worker crate.

Contribution requirements are defined in the ReallyMe
[engineering philosophy](https://github.com/reallyme/.github/blob/main/ENGINEERING_PHILOSOPHY.md)
and [architecture standards](https://github.com/reallyme/.github/blob/main/ARCHITECTURE.md).
