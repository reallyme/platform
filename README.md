<div align="center">

# Platform

**Application architecture for taking Rust services from framework to production.**

[![CI](https://github.com/reallyme/platform/actions/workflows/ci.yml/badge.svg)](https://github.com/reallyme/platform/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/reallyme-platform.svg?label=crates.io)](https://crates.io/crates/reallyme-platform)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE)

**[Getting started](#getting-started) · [Applications](#applications) · [Kits](#kits) · [Architecture](#architecture) · [Hephaestus](#hephaestus)**

</div>

---

Platform provides host-neutral Rust application contracts, a native server runtime,
and optional infrastructure integrations. Applications expose HTTP, Connect RPC,
gRPC, or Cloudflare Workers adapters as required by the host.

## Getting started

Clone the repository and run the reference native server:

```console
git clone https://github.com/reallyme/platform.git
cd platform
cargo run -p example-server -- \
  --config servers/configs/example-server.jsonc
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
[reference native server](servers/example-server/) shows explicit application
selection and process composition. The
[reference Cloudflare Workers host](workers/example-worker/) supplies
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
[Buffa](https://github.com/anthropics/buffa) generates Rust message types and
borrowed views, while Connect RPC provides the primary RPC transport.

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

Hephaestus sits beside these layers as a deployment control plane. It
coordinates the application of desired state but is not required by the
application, server, or runtime model.

The public repository is organized around reusable kits, first-party components, and reference implementations:

```text
platform/
├── kits/
│   ├── reallyme-app-kit/
│   ├── reallyme-server-kit/
│   ├── reallyme-foundationdb-kit/
│   ├── reallyme-nats-kit/
│   ├── reallyme-postgres-kit/
│   ├── reallyme-s3-kit/
│   ├── reallyme-typesense-kit/
│   └── reallyme-valkey-kit/
├── components/
│   └── hephaestus/
│       ├── domain/
│       ├── contract/
│       └── agent/
├── apps/
│   └── example/
├── servers/
│   ├── configs/
│   │   └── example-server.jsonc
│   └── example-server/
└── workers/
    └── example-worker/
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for the detailed runtime model.

## Kits

Platform kits provide reusable, typed boundaries for application hosting and
infrastructure services. They own connection lifecycle, validation, readiness,
bounded operations, and safe error classification. Applications continue to
own their schemas, payloads, tenant policy, and business behavior.

### Application and runtime

| Kit | Provides |
| --- | --- |
| [`reallyme-app-kit`](kits/reallyme-app-kit/) | Host-neutral application metadata, configuration, health, lifecycle contributions, permissions, metrics naming, and adapter conventions. |
| [`reallyme-server-kit`](kits/reallyme-server-kit/) | Native listeners, startup and shutdown, readiness, tracing, metrics, task supervision, HTTP, Connect RPC, optional gRPC, and WebSockets. |

### Data and infrastructure

| Kit | Service | Provides |
| --- | --- | --- |
| [`reallyme-foundationdb-kit`](kits/reallyme-foundationdb-kit/) | FoundationDB | Process-scoped client lifecycle, validated tenant handles, transaction and retry policies, tuple and range helpers, readiness, and optional tenant administration. |
| [`reallyme-postgres-kit`](kits/reallyme-postgres-kit/) | PostgreSQL | TLS-first connection pooling, bounded timeouts, typed transactions, retry classification, migration locks, and readiness. |
| [`reallyme-valkey-kit`](kits/reallyme-valkey-kit/) | Valkey | TLS-first connections, bounded commands and pipelines, namespaced binary keys, mandatory TTL policy, leases, counters, and readiness. |
| [`reallyme-typesense-kit`](kits/reallyme-typesense-kit/) | Typesense | Validated endpoints and API keys, failover and retry policy, collection lifecycle, typed search and multi-search, bulk import, and readiness. |
| [`reallyme-nats-kit`](kits/reallyme-nats-kit/) | NATS and JetStream | TLS-aware connections, bounded publishing, acknowledgements, deterministic deduplication, and durable pull consumers. |
| [`reallyme-s3-kit`](kits/reallyme-s3-kit/) | S3-compatible object storage | Endpoint and object-key validation, AWS Signature Version 4, bounded uploads, and native or Cloudflare Workers clients. |

Connector kits deliberately stop at the infrastructure boundary. Product event
subjects, database schemas, collection definitions, object naming policy, and
serialization stay with the application that owns them.

## Hephaestus

Platform bundles the Hephaestus node agent as a first-party operational
component. Hephaestus is ReallyMe's deployment path for Platform installations,
but applications and servers do not depend on it and may be deployed through
another system.

The agent, its wire contract, and its host-neutral domain types live together
under [`components/hephaestus/`](components/hephaestus/). The Hephaestus
controller consumes those versioned components while remaining a separate
application. This keeps the Platform checkout buildable without private
controller or company infrastructure code.

## Status

Platform is actively developed and used in production at ReallyMe.

Releases follow semantic versioning. Before 1.0, public APIs may evolve between
minor releases; compatibility-impacting changes are documented in release notes.

## License

Licensed under either the [MIT License](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option.

Third-party components retain their own licenses and notices.

## Copyright And Trademarks

Copyright © 2026 by ReallyMe Technologies Limited.

ReallyMe® is a registered trademark of ReallyMe LLC.

Cloudflare and Cloudflare Workers are trademarks and/or registered trademarks
of Cloudflare, Inc.
