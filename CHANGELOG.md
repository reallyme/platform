# Changelog

## 0.4.0

- Use ReallyMe Buffa 0.1.2 for generated contracts and the example Connect
  transport. Protobuf field numbers and RPC paths are unchanged.
- Encode native and Worker HTTP responses with ProtoJSON while preserving
  published JSON shapes.
- Write protobuf tenant metadata, reservations, and deletion intents while
  retaining readers for historical formats. Upgrade application readers and
  operator binaries together before enabling 0.4.0 writes on a shared cluster.
- Add durable deletion, recovery, takeover, and tombstone reclamation to
  `fdb-tenant-admin`. Recovery verifies the original tenant ID after name reuse.
- Enforce transaction budgets before callbacks and commit submission. Reject
  duplicate gRPC timeout metadata.
- Add `FdbSetupErrorReason::ClusterIdentityUnavailable` and
  `GrpcDeadlineErrorReason::DuplicateTimeout`; update exhaustive matches.
- Require Rust 1.100.0. Update ReallyMe Crypto to 0.4.2 and Worker runtime/build
  tools to 0.9.0.

## 0.3.5

- Support Typesense 30 collection schema responses and omit absent default sort
  fields from collection creation.
- Test service compatibility with NATS 2.15.2, PostgreSQL 18, Typesense 30.2,
  and Valkey 9.1.3.
- Refresh compatible Rust dependencies and Worker tooling.

## 0.3.4

- Update ReallyMe Codec to 0.3.2 and Crypto to 0.3.17.
- Require Rust 1.99.1 and refresh compatible dependencies.

## 0.3.3

- Update `reallyme-crypto` and its component crates to 0.3.13.

## 0.3.2

- Preserve the 0.3.1 `websocket_same_origin` signature; use
  `websocket_same_origin_with_external_origin` for URI-aware checks.
- Release closed gRPC connection tasks promptly. Regenerate inbound gRPC request
  and trace IDs; callers must read server-generated IDs from responses or logs.
- Bound rate-limit source churn without resetting existing token debt. New
  sources share bounded overflow allowances. Overcommitted tiers share the
  registry capacity independently of registration order.
- Preserve active HTTP/2 WebSockets during keep-alive retirement and permit
  progressing socket writes. Validate configurable HTTP transport deadlines.
- Add per-listener gRPC keepalive, first-request, idle, age, and age-grace
  deadlines. Retirement sends GOAWAY and permits draining until the grace cap.
- Preserve typed WebSocket close codes 1002, 1007, and 1009. Add a bounded
  message-handler deadline separate from outbound socket-write limits.
- Align Worker Connect oversized-body responses with native status 413 and
  resource-exhausted errors. Permit browser Connect preflight headers.
- Require raw and decoded paths to select the same route-visibility rule;
  reject ambiguous separators, percent signs, NUL, dot segments, and malformed
  escapes. Accept validated forwarded-client chains with ports and unspecified
  identities; use the nearest proxy's external host and scheme.
- Close over-cap sockets without blocking the listener accept loop.
- Add `fdb-tenant-admin id` and `recover-delete`. Deletion attempts metadata
  restoration on failure; manual recovery requires a trusted recorded tenant ID
  and an empty reserved metadata namespace. Interrupted provisioning still
  requires explicit repair of an empty tenant.
- Reject read-policy mutations even when the callback ignores the returned
  error, while preserving the existing data-error enum and `clear` signatures.
- Ignore discovered servers for explicit plaintext NATS connections. Native TLS
  clients use host roots, falling back to bundled roots only when the host store
  is empty. App-kit HTTPS, S3, Typesense, and NATS accept exclusive custom roots.
- Install ring for Valkey TLS when the host has not selected a crypto provider.
- Require HTTPS for service locators and Tailscale resolution. Accept validated
  multi-label custom DNS suffixes. Reject bare CR line endings in JSONC.
- Use the active shutdown mode's listener drain budget. Join overdue HTTP
  connection tasks before app cleanup.
- Serve HTTP during critical-task startup; `/readyz` remains unavailable until
  startup completes. Liveness, app routes, and `/metrics` can respond meanwhile.
  Saturated gRPC methods return `RESOURCE_EXHAUSTED`.
- Load Worker protobuf DTOs without the native Connect router dependency and
  refresh compatible dependencies.

## 0.3.1

- Exempt trusted proxies and loopback sidecars from the per-source TCP cap while
  retaining the global cap. Make both limits configurable per listener.
- Close excess sockets promptly. Retire idle HTTP/gRPC connections after 60
  seconds and stalled HTTP writes after 30 seconds, retaining the drain window.
- Bound rate-limit source tables and configure IPv6 grouping from `/48` through
  `/128`, with `/64` as the default.
- Use route-wide admission for WebSockets without a source IP. A failed HTTP
  connection task no longer terminates its listener; age retirement drains
  connections gracefully.
- Reject ambiguous paths and multiple `Forwarded` field lines from trusted peers.
- Refuse FoundationDB deletion when application data remains and clear kit
  metadata before removing an empty tenant.
- Use the supported Worker header API and include both licenses in crate archives.

## 0.3.0

- The facade has no default integrations. Select `server-connect` and
  `server-websocket` separately from `native-server`. `s3` exposes contracts;
  clients require `s3-native-client` or `s3-worker-client`.
- FoundationDB connection initialization is `unsafe`: retain the network
  runtime for all handles and drop them before process exit. Application
  transactions use validated keys/ranges; tenant administration requires
  `tenant-admin`, and the raw database accessor is removed.
- Construct server-kit `CorsConfig` through its methods rather than matching
  enum variants. Non-loopback HTTP origins are rejected.
- `AppPortError` is non-exhaustive and no longer carries provider-specific
  variants. App-kit HTTPS requires `native-http`.
- Construct Typesense clients through validated `TypesenseConnector` config.
  Search builders use `SearchFields`, `SearchQueryWeights`, and `CollectionName`;
  multi-search and filter combinators are fallible.
- Native S3 client construction is fallible. Signed requests borrow validated
  inputs. Native S3 and Typesense require usable TLS trust roots.
- NATS requires TLS by default; plaintext opt-in is limited to local development.
  Test helpers require `testing`. Minimal native containers need a CA bundle or
  `SSL_CERT_FILE`.
- HTTP listeners reject gRPC and gRPC-Web content types; use a gRPC listener.
  Add global/per-source TCP caps, first-request deadlines, and stricter proxy
  and route policies.
- `/healthz` waits for critical application startup. Version 0.3.2 changes this
  behavior; use `/readyz` to gate traffic.
- Require Rust 1.99.0.
