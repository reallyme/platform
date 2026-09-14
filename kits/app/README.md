# reallyme-app-kit

`reallyme-app-kit` defines host-neutral app contracts and conventions for
ReallyMe app crates.

It is intentionally not a server runtime. It does not bind listeners, initialize
Tokio, mount Axum routes, register tonic services, install tracing, or own
shutdown. Those responsibilities belong to host runtimes such as
`reallyme-server-kit` or future Workers host adapters.

App crates use this kit for reusable, typed conventions:

- app metadata and registration descriptors
- app-owned config validation interface
- standard app JSONC config envelope parsing and validation
- health contribution contracts
- startup check, cleanup hook, and background task contracts
- app permission naming primitives
- app metric namespace/name validation
- host/transport adapter and port naming conventions

Connect/gRPC-shaped app contracts use Buf/protobuf as the canonical schema
source. Per the Connect Rust guide, product apps must use `buf generate` for
Rust RPC generation before Rust formatting and checks. Do not hide generation
behind Cargo `build.rs`. Generated output belongs only in the owning contract
crate and must be checked in so published crates and clean source checkouts are
reproducible. The standard Connect generation path uses `protoc-gen-buffa` for
message/view types, `protoc-gen-connect-rust` for Connect service stubs, and
`protoc-gen-buffa-packaging` for module trees.
App Connect adapters should expose Tower-native `connectrpc::Router` or
`tower::Service` values. Axum, Hyper, Workers, or other hosts adapt those
routers at the host/adapter boundary; app core logic must not depend on a
specific native HTTP framework.

In Connect/protobuf terminology, a `service` is the generated Rust trait for
one RPC surface. In ReallyMe architecture, an app may contain one or more of
these RPC services. The protobuf service is not the deployed server process,
runtime, listener owner, container, or lifecycle unit.

New applications should use `apps/example` as the maintained reference for
app-kit contracts, generated contract ownership, and host-neutral core logic.

The standard app JSONC envelope is owned by app-kit:

- `public_base_url`
- `cors.allowed_origins`
- `cookies.secure`
- `cookies.domain`
- `cookies.same_site_policy`
- `reflection_enabled`
- `downstream.<port_name>.base_url`
- `downstream.<port_name>.endpoints`
- `downstream.<port_name>.endpoint_selection`
- `downstream.<port_name>.locator`

Apps may add custom top-level properties through typed custom config, but they
should not redefine the standard envelope.

Downstream endpoint config supports a singleton `base_url`, a static
`endpoints` list, or a host-resolved `locator`. Native server hosts can resolve
`locator.mode = "tailscale_service"` through Tailscale Services/MagicDNS
without app core depending on server-kit or Tailscale APIs.

Each enabled app should have exactly one explicit app config source. In
ReallyMe server composition, the host points at that app-owned JSONC document
with `apps[].config_path`, or injects it intentionally with
`apps[].config_inline_jsonc`. The server JSONC owns process/runtime concerns;
the app JSONC owns app behavior.

`reflection_enabled` is reserved for future host-reviewed schema exposure
policy. It must remain `false` today; app-kit validation rejects `true` so apps
cannot quietly request gRPC/Connect introspection from config.

The intended layering is:

- app core depends on `reallyme-app-kit`, its app-owned domain, and app-owned ports
- server adapters bridge app behavior into `reallyme-server-kit`
- app Workers adapters bridge the same app behavior into concrete Worker hosts
- product/domain semantics stay in their owning application or product repository

This crate must not depend on `reallyme-server-kit`, Axum, Tonic, Tokio runtime
ownership, database clients, Cloudflare APIs, or process lifecycle concepts.
Concrete Worker host crates under `workers/` own `workers-rs`, `wrangler.jsonc`,
and Wasm build/runtime concerns.
