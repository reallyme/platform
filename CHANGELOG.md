# Changelog

## 0.3.2

- Refresh compatible Rust dependencies and the workspace lockfile, and update
  Worker tooling to pnpm 12.9.1.
- Rate-limit source churn no longer grants a fresh token while existing buckets
  retain debt. New sources share bounded overflow capacity until a tracked
  bucket has fully refilled. A bounded candidate search admits newcomers when
  a refilled bucket follows an older source that still owes tokens.
- HTTP/2 WebSocket upgrades survive HTTP keep-alive retirement, and slow
  response readers continue while socket writes progress. Flow-control stalls
  release the connection within the configured write-stall deadline.
- Native gRPC sends GOAWAY before its idle socket fallback closes a connection;
  active streams retain a separate grace period after GOAWAY.
- Route visibility evaluates a single decoded path: ordinary escaped characters
  are accepted, while encoded separators, percent signs, NUL, and dot segments
  receive a bad-request response.
- Over-cap sockets no longer pause the listener's shared accept loop. Invalid
  forwarded client chains are rejected rather than assigned to the proxy's
  rate-limit identity.
- FoundationDB delete attempts bounded metadata restoration after any failed
  phase. Operators can recreate both missing keys with `recover-delete` after
  verifying tenant identity, even when application data remains; recovery
  records a new metadata creation time.
  Application transactions opened with a read policy reject mutations.
- Explicit plaintext NATS connections ignore discovered servers. Native TLS
  clients include bundled public roots alongside OS roots; HTTP clients offer
  HTTP/2 and HTTP/1.1 through ALPN.
- HTTP transport deadlines are configurable through validated server settings.
  JSONC line comments now preserve CR-only line endings.
- HTTP listeners answer liveness requests while application critical tasks are
  still becoming ready. Overdue HTTP connection tasks are cancelled and joined
  before application cleanup starts. A saturated gRPC method responds with
  `RESOURCE_EXHAUSTED` instead of reaching the first-request transport deadline.
- WebSocket message handlers have a bounded execution deadline. Browser Connect
  preflight permits protocol and timeout headers on both reference hosts.
- The reference Worker loads protobuf DTOs without the native Connect router
  dependency. Authenticated Typesense integration coverage exercises a collection
  read in addition to its public health endpoint.

## 0.3.1

- Trusted proxies and local sidecars no longer share the per-source TCP connection cap.
  Both TCP admission limits can be configured per listener; the global limit still applies.
- Full listeners promptly close excess sockets instead of leaving clients in the accept
  backlog. Idle HTTP and gRPC connections retire after 60 seconds, and stalled HTTP writes
  fail after 30 seconds; active streams retain their drain window.
- Full rate-limit source tables use indexed eviction of their least recently admitted
  source while keeping a bounded table and capacity for existing tiers. A replacement receives one
  initial token rather than a fresh burst. Per-source tiers can configure IPv6 source
  grouping from `/48` through `/128`; the default remains `/64`.
- WebSocket connections without a known source IP use the configured route-wide limit
  instead of a shared 64-connection source bucket.
- A failed HTTP connection task no longer terminates its listener. HTTP connection age
  now uses a graceful protocol shutdown; WebSockets retain their close-frame policy.
- Ambiguous HTTP paths are rejected before route visibility and resource policies run.
  The selected `Forwarded` header family rejects multiple field lines from trusted peers.
- FoundationDB tenant deletion clears kit metadata before deleting an empty tenant and
  reports application data that prevents deletion.
- The example Worker reads ordinary headers through the supported `Headers.get` API.
- Published crate archives include both license texts.

## 0.3.0

### Breaking API and feature changes

- The `reallyme-platform` facade starts with no default integrations. Its `s3` feature
  exposes S3 contracts; native and Worker clients require `s3-native-client` and
  `s3-worker-client`, respectively. `native-server` no longer enables Connect or
  WebSocket support; select `server-connect` or `server-websocket` explicitly.
- FoundationDB connection initialization is now `unsafe` because callers must keep
  the process-wide network runtime alive. The raw administrative database accessor
  was removed from the tenant API.
- Server-kit `CorsConfig` changed from a public enum to a validated type.
  Construct it with its methods and inspect it through accessors instead of
  matching variants. Non-loopback HTTP CORS origins are now rejected.
- App-kit `AppPortError` is non-exhaustive and no longer carries provider-specific
  variants. Native HTTP integration requires the `native-http` feature, so app
  cores can build without a native HTTP client dependency.
- Typesense callers construct a `TypesenseConnector` from validated config;
  direct `TypesenseClient::new` use is no longer public. Search builders use
  `SearchFields`, `SearchQueryWeights`, and `CollectionName`, while bounded
  multi-search and filter combinators return typed results.
- S3 native client construction is fallible, and signed requests borrow their
  validated inputs. Native S3 and Typesense clients require usable certificate
  roots when constructing TLS clients. The S3 facade separates contracts from
  native and Worker clients, so select the intended client feature.
- NATS test helpers require the `testing` feature. The TLS policy and credential
  configuration are validated at the connector boundary.
- FoundationDB tenant administration is isolated behind `tenant-admin`; data
  access uses validated tenant keys and ranges instead of a raw database handle.
- The minimum supported Rust version is 1.99.0.

### Behavior changes

- NATS now requires TLS by default. Explicit plaintext opt-in is limited to local
  development endpoints; the `testing` module requires its feature flag.
- Native TLS clients load trust roots from the operating system. Minimal container
  images need a CA bundle or an appropriate `SSL_CERT_FILE` configuration.
- HTTP listeners reject gRPC and gRPC-Web content types; use a gRPC listener for
  those transports.
- Server listeners gained global and per-source TCP admission limits, first-request
  deadlines, and stricter forwarded-header and route visibility policies.
- `/healthz` is served only after critical application startup tasks are ready.
