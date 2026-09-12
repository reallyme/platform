# Platform Architecture

Platform is the layer between a Rust application and the environment that runs
it. It gives applications a stable shape without requiring them to own a
listener, an async runtime, process signals, telemetry initialization, or an
edge entrypoint.

The central rule is simple:

> The application owns behavior. The host owns execution.

That rule is what makes the same application core usable through Connect RPC,
HTTP, gRPC, a native server, or Cloudflare Workers.

## The Model

```text
application contract ──defines boundary──> transport adapter
external caller ──────────────────────────> transport adapter ──> application core
native or edge host ──────────────────────> host integration ───> application core

application core ──calls──> app-owned outbound port
                                      ▲
                                      │ implements
                    application infrastructure adapter
                                      │
                                      └──uses──> infrastructure kit
```

An application can support only the boundaries it needs. A host selects which
applications and adapters are compiled into its artifact. Application-specific
infrastructure adapters implement the ports required by the core; reusable kits
provide the underlying infrastructure primitives without acquiring product
policy.

## Vocabulary

**Application**
: A business or use-case capability. Its core is host-neutral and testable
  without a production runtime.

**Contract**
: The durable interface an application exposes to callers. RPC-shaped
  contracts are defined in Protocol Buffers and compiled into a separate Rust
  crate. A contract crate may also expose a host-neutral callable port for the
  same inbound boundary.

**Port**
: An application-owned interface at a dependency boundary. An inbound contract
  port exposes application behavior to callers. An outbound capability port
  represents something the core requires, such as a repository, clock, queue
  publisher, or another application.

**Adapter**
: Code that translates between an application boundary and a transport,
  runtime, database, queue, object store, or external service.

**Host**
: The executable environment that runs one or more applications. Native
  servers and Cloudflare Workers are hosts.

**Kit**
: A reusable Platform crate that standardizes an application, runtime, or
  infrastructure boundary. Kits do not contain product policy.

## Repository Structure

The public repository is organized around reusable kits, first-party
components, and reference hosts:

```text
platform/
├── src/                  # The reallyme-platform facade crate
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
│   └── example-server/
└── workers/
    └── example-worker/
```

Private applications are not members of this workspace. They consume Platform
as versioned dependencies and remain outside its public surface.

The root `reallyme-platform` package is a thin facade. It re-exports the
host-neutral application kit, enables the native server kit by default, and
exposes infrastructure and Hephaestus kits through opt-in features. It does not
wrap or replace the underlying crate types, and each kit remains independently
usable.

## Application Boundary

Application cores own use cases, domain validation, application state, and the
ports required to perform their work. They may use `reallyme-app-kit` for
Platform's host-neutral application interfaces, lifecycle contracts, and
conventions.

Application cores do not depend on Axum, Tonic, Connect implementations,
`reallyme-server-kit`, Cloudflare Workers runtime APIs, listeners, shutdown
signals, or process readiness. Those dependencies enter through explicit
adapters.

A typical application has these boundaries:

```text
my-app/
├── contract/
│   ├── proto/
│   └── src/generated/
├── src/
│   ├── app/
│   ├── ports/
│   └── adapters/
│       ├── connect/
│       ├── http/
│       ├── grpc/
│       └── server/
└── tests/
```

Adapter directories exist only for supported transports and hosts. A native
server adapter returns composition values to the host; it does not start a
listener or take ownership of the runtime. Cloudflare-specific request,
response, binding, and entrypoint adapters belong to the Worker host.

## Contracts and Transports

Protocol Buffers are the source of truth for RPC-shaped application contracts.
Buf supplies schema linting, drives code generation, and provides compatibility
tooling for breaking-change checks as contracts evolve. Buffa generates Rust
message and view types, and the Connect generator produces the primary RPC
bindings. Generated source is isolated in the contract crate and is never
edited by hand.

Generated messages are wire representations, not domain authority. Transport
adapters decode external input and perform boundary validation before converting
it into application or domain types. Domain invariants remain owned by the
application.

Connect RPC is the primary RPC transport. HTTP/JSON is a compatibility adapter
unless an application is intentionally HTTP-only. Standalone Tonic gRPC is
optional and explicit. Each transport calls the same application use cases;
transport adapters do not implement parallel business logic or depend on one
another.

When one application calls another, it depends on the target application's
contract crate while expressing the dependency through an outbound port owned
by the calling application. A concrete client adapter implements that port by
using the target contract's callable interface. The binding may be in-process
or remote without changing the calling core.

## Native Servers

A native server is an explicit, compile-time composition. It chooses the
applications in the artifact, their adapters, and the concrete implementation
of every required port. Configuration can enable or configure compiled
applications, but it cannot introduce application code that was not built into
the server.

`reallyme-server-kit` owns the common process machinery:

- configuration loading and validation;
- Tokio runtime integration and managed task supervision;
- listener binding and transport hardening;
- startup sequencing and readiness publication;
- standard liveness, readiness, version, and metrics endpoints;
- tracing and metrics initialization;
- cancellation, graceful shutdown, and bounded termination.

Startup is phased. Configuration is validated before external resources are
opened. Required dependencies become ready before traffic is admitted.
Readiness is revoked before shutdown begins. Managed tasks receive
cancellation, drain within their deadlines, and report typed failures to the
runtime.

Applications may contribute readiness checks and managed background work, but
they do not own the process lifecycle that executes them.

## Cloudflare Workers

A Worker is another host composition, not a second application
implementation. Its entrypoint owns Cloudflare bindings, request and response
translation, and the lifecycle supplied by the Workers platform. It calls the
same host-neutral core used by a native server.

Worker hosts do not emulate native process concerns that the edge platform
already owns. They still preserve the same contract validation, typed errors,
bounded work, observability, and application boundary.

## Configuration and Composition

Applications own typed configuration models for their behavior. Hosts own the
configuration document, secret resolution, environment integration, and the
decision to activate a compiled application.

Configuration follows the same progression as request input:

```text
JSONC and environment values
            │
            ▼
    host-level parsing
            │
            ▼
typed application configuration
            │
            ▼
validated runtime values
```

Secrets are supplied through secret providers or host bindings. They are not
embedded in source configuration, included in diagnostics, or exposed through
generic debug output.

## Infrastructure Kits

Infrastructure kits provide typed, reusable integration primitives for services
such as FoundationDB, PostgreSQL, NATS, Typesense, Valkey, and S3-compatible
object storage. A kit may provide connection lifecycle, retry classification,
bounded operations, timeout policy, readiness integration, and testing fakes.

A kit does not own application schemas, product event subjects, collection
policy, tenant policy, ranking behavior, or business-specific data models.
Those remain with the application that gives them meaning.

## Platform and Product Domains

Platform-domain types are limited to concepts intrinsic to Platform itself:
application identity, capabilities, lifecycle contributions, health, host
integration, and similar cross-host contracts.

Product-domain types remain with the company or application that owns them. A
public Platform crate must never depend on a product domain. Shared product
types remain outside Platform unless the concept itself is intrinsic to
Platform.

## Hephaestus

The Hephaestus node agent is distributed as a first-party Platform component
and is part of ReallyMe's deployment path. It remains separate from Platform's
application and runtime model. Its host-neutral domain and wire contract live
beside the agent so the public component has no dependency on a private
controller repository.

A Platform application does not depend on Hephaestus and may be deployed by
another system. Company infrastructure repositories remain the source of truth
for desired state, topology, credentials, and environment policy.

## Dependency Direction

Dependencies point toward stable, host-neutral boundaries:

```text
Inbound behavior

external caller ──> transport adapter ──> application core
native host ──────> server integration ──> application core
Worker host ──────> Worker adapter ──────> application core
tests ───────────────────────────────────> application core

Outbound capabilities

application core ──calls──> app-owned port
                                    ▲
                                    │ implements
                   application infrastructure adapter
                                    │
                                    └──uses──> infrastructure kit
```

Contract crates define stable inbound wire boundaries and may expose
host-neutral callable ports for in-process and remote client adapters. They do
not depend on application implementations or hosts. Outbound capabilities
required by a core are defined by ports owned by that core's application. Host
code may know about applications. Applications must not know which server,
Worker, company, cluster, or deployment system hosts them. Platform kits must
not depend on private applications.

## Architectural Test

Before adding a dependency or responsibility, ask:

1. Does it describe business behavior, a transport, a host, or infrastructure?
2. Can the application core still compile without a native or edge runtime?
3. Could this application be hosted somewhere we have not designed yet?
4. Can another host reuse the behavior without importing the first host?
5. Does the dependency point toward the more stable boundary?
6. Is a supposedly reusable kit acquiring product policy?

If the answer is unclear, make the boundary explicit before adding code.
