<div align="center">

# Platform

**Application architecture for taking Rust services from framework to production.**

[![CI](https://github.com/reallyme/platform/actions/workflows/ci.yml/badge.svg)](https://github.com/reallyme/platform/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/reallyme-platform.svg?label=crates.io)](https://crates.io/crates/reallyme-platform)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

**[Getting started](#getting-started) · [Applications](#applications) · [Kits](#kits) · [Architecture](#architecture)**

</div>

---

Platform lets you write host-neutral Rust applications and expose them through
HTTP, Connect RPC, gRPC, native servers, or Cloudflare Workers® without
coupling business logic to a particular transport or runtime.

Keep Tokio, Axum, Tonic, Cloudflare Workers, and the libraries you already use.
Platform supplies the application model, native runtime, configuration, health,
observability, and infrastructure kits that turn those pieces into a coherent
production system.

Run the same application core at the edge, in a container, on Kubernetes, or
on your own infrastructure. Change deployment models without rewriting your
business logic.

**Write the application once. Choose where it runs later.**

## Why Platform

Rust has excellent frameworks. The difficult part starts when an application becomes a production service.

Listeners need to start and stop correctly. Configuration must be loaded and
validated. Health and observability need consistent behavior. RPC contracts
need to evolve without drifting between transports. The same application may
need to run in a container today and at the edge tomorrow.

Those concerns tend to accumulate inside every service:

```text
application
├── business logic
├── Axum router
├── gRPC server
├── listener
├── shutdown
├── tracing
├── configuration
└── health
```

Platform separates the application from the host that executes it:

```text
                 Application core
                        │
          ┌─────────────┼─────────────┐
          │             │             │
        HTTP         Connect         gRPC
          │             │             │
          └─────────────┼─────────────┘
                        │
                 Host integration
                        │
              ┌─────────┴─────────┐
              │                   │
        Native runtime       Edge runtime
```

The application owns behavior. The host owns execution.

Application code does not own listeners, shutdown signals, tracing
initialization, process readiness, or production edge entrypoints. Native
servers compose applications through Platform's runtime; edge adapters run the
same application core under the host platform's lifecycle.

## Getting started

Add the facade crate to use Platform's host-neutral application contracts:

```console
cargo add reallyme-platform
```

Infrastructure integrations are opt-in features:

```console
cargo add reallyme-platform --features postgres,nats
```

The native runtime is also opt-in:

```console
cargo add reallyme-platform --features native-server
```

Clone the repository and run the reference native server:

```console
git clone https://github.com/reallyme/platform.git
cd platform
cargo run -p example-server -- \
  --config servers/example/config/example-server.jsonc
```

In another terminal, call the reference application and the runtime-owned
readiness endpoint:

```console
curl http://127.0.0.1:8080/hello
curl http://127.0.0.1:8080/readyz
```

The [reference application](apps/example/) includes host-neutral behavior, a
protobuf contract, Connect and HTTP adapters, native-server integration, and a
core that can be called without a native runtime. The
[reference native server](servers/example/) shows explicit application
selection and process composition. The
[reference Cloudflare Workers host](workers/example/) supplies
the Cloudflare-specific adapter and shows how the same application runs on
Cloudflare Workers.

Use the feature matrix to verify that the core remains independent of every host:

```console
cargo check -p example-app --no-default-features
cargo check -p example-app --no-default-features --features connect
cargo check -p example-app --no-default-features --features native-server
cargo check -p example-worker
```

## Applications

A Platform application is a capability, not a process.

Its core logic is host-neutral. Transport and host integrations live at the
boundary and adapt external requests into application behavior. A typical
application repository looks like this:

```text
my-app/
├── Cargo.toml
├── Cargo.lock
├── config/
├── contract/
│   ├── Cargo.toml
│   ├── buf.yaml
│   ├── proto/
│   └── src/
│       └── generated/
├── src/
│   ├── app/
│   ├── ports/
│   └── adapters/
│       ├── connect/
│       ├── http/
│       └── server/
└── tests/
```

Adapter directories are present only when the application supports that
transport or host. The application core compiles without Axum, Tonic, Connect,
server-kit, or a Cloudflare Workers runtime. Those dependencies enter through
explicit adapters. Cloudflare-specific adapters belong to the Worker host, as
shown by the reference Worker.

Servers explicitly select the applications they contain. Runtime configuration
may activate and configure applications compiled into an artifact, but it
cannot introduce new application code. Infrastructure selects where an
immutable artifact runs; it does not change the applications compiled into it.

The result is deterministic server composition with one authoritative lockfile
and a traceable path from deployed artifact back to source.

## Contracts and RPC

Platform treats application contracts as durable interfaces, not Rust implementation details.

Applications define RPC-shaped contracts in Protocol Buffers.
[Buf](https://buf.build/) supplies schema linting, code generation, and
compatibility tooling for breaking-change checks.
Buffa generates Rust message types and borrowed views, while Connect RPC
provides the primary RPC transport.

We use Protocol Buffers because an application contract should outlive any
particular transport, host, or Rust implementation. Stable field numbers,
language-independent schemas, and explicit compatibility rules give
applications a contract that can evolve independently of where they run. Buf
makes that model practical to enforce as contracts evolve.

```proto
syntax = "proto3";

package example.greeter.v1;

service GreeterService {
  rpc Greet(GreetRequest) returns (GreetResponse);
}

message GreetRequest {
  string name = 1;
}

message GreetResponse {
  string message = 1;
}
```

From that source, `buf generate` produces the Rust wire types and Connect
bindings used at the application boundary. HTTP/JSON and standalone gRPC remain
explicit adapters over the same application behavior.

Generated types are wire representations, not domain authority. Adapters
validate external input and convert it into application or domain types before
invoking core behavior. When one application calls another, it depends on the
target application's contract crate rather than its implementation. Calls pass
through an application-owned port and a concrete client adapter, keeping the
core independent of transport and deployment topology.

## Architecture

Platform has four deliberate boundaries:

| Layer | Responsibility |
| --- | --- |
| Platform | Defines how applications integrate with hosts and infrastructure services. |
| Applications | Own business capabilities, contracts, ports, and adapters. |
| Servers and edge hosts | Select applications and produce concrete executable artifacts. |
| Infrastructure | Decides where immutable artifacts run and how they are operated. |

The public repository is organized around reusable kits and reference implementations:

```text
platform/
├── crates/
│   └── platform/         # Least-dependency reallyme-platform facade
├── kits/
│   ├── app/
│   ├── server/
│   ├── foundationdb/
│   ├── nats/
│   ├── postgres/
│   ├── s3/
│   ├── typesense/
│   └── valkey/
├── apps/
│   └── example/
├── servers/
│   └── example/
│       └── config/
│           └── example-server.jsonc
└── workers/
    └── example/
```

See [the architecture guide](docs/concepts/architecture.md) for the detailed
runtime model and ownership rules.

## Kits

Platform kits provide reusable, typed boundaries for application hosting and
infrastructure services. They own connection lifecycle, validation, readiness,
bounded operations, and safe error classification. Applications continue to
own their schemas, payloads, tenant policy, and business behavior.

### Application and runtime

| Kit | Provides |
| --- | --- |
| [`reallyme-platform`](crates/platform/) | Least-dependency facade over the application kit and explicitly selected runtime and infrastructure integrations. |
| [`reallyme-app-kit`](kits/app/) | Host-neutral application metadata, configuration, health, lifecycle contributions, permissions, metrics naming, and adapter conventions. |
| [`reallyme-server-kit`](kits/server/) | Native listeners, startup and shutdown, readiness, tracing, metrics, task supervision, HTTP, Connect RPC, optional gRPC, and WebSockets. |

### Data and infrastructure

| Kit | Service | Provides |
| --- | --- | --- |
| [`reallyme-foundationdb-kit`](kits/foundationdb/) | FoundationDB | Process-scoped client lifecycle, validated tenant handles, transaction and retry policies, tuple and range helpers, readiness, and optional tenant administration. |
| [`reallyme-postgres-kit`](kits/postgres/) | PostgreSQL | TLS-first connection pooling, bounded timeouts, typed transactions, retry classification, migration locks, and readiness. |
| [`reallyme-valkey-kit`](kits/valkey/) | Valkey | TLS-first connections, bounded commands and pipelines, namespaced binary keys, mandatory TTL policy, leases, counters, and readiness. |
| [`reallyme-typesense-kit`](kits/typesense/) | Typesense | Validated endpoints and API keys, failover and retry policy, collection lifecycle, typed search and multi-search, bulk import, and readiness. |
| [`reallyme-nats-kit`](kits/nats/) | NATS and JetStream | TLS-aware connections, bounded publishing, acknowledgements, deterministic deduplication, and durable pull consumers. |
| [`reallyme-s3-kit`](kits/s3/) | S3-compatible object storage | Endpoint and object-key validation, AWS Signature Version 4, bounded uploads, and native or Cloudflare Workers clients. |

Connector kits deliberately stop at the infrastructure boundary. Product event
subjects, database schemas, collection definitions, object naming policy, and
serialization stay with the application that owns them.

## Status

Platform is actively developed and used in production at ReallyMe.

Releases follow semantic versioning. Before 1.0, public APIs may evolve between
minor releases; compatibility-impacting changes are documented in release notes.

## License

Licensed under either the [MIT License](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option.

Third-party components retain their own licenses and notices.

## Copyright And Trademarks

Copyright © 2026 by ReallyMe LLC.

ReallyMe® is a registered trademark of ReallyMe LLC.

Cloudflare and Cloudflare Workers are trademarks and/or registered trademarks
of Cloudflare, Inc.
