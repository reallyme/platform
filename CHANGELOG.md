# Changelog

## 0.3.2

- Refresh compatible Rust dependencies and the workspace lockfile, and update
  Worker tooling to pnpm 12.9.1.
- Rate-limit source churn no longer grants a fresh token while existing buckets
  retain debt. New sources use fixed overflow shards with a separate, bounded
  allowance of twice the tier's burst and refill budget. A low-cardinality
  counter reports allowed and rejected overflow decisions. A bounded candidate
  search admits newcomers when a refilled
  bucket follows an older source that still owes tokens. Overcommitted source
  caps are reported at registry construction; the global 25,000-bucket cap wins.
- HTTP/2 WebSocket upgrades survive HTTP keep-alive retirement, and slow
  response readers continue while socket writes progress. Flow-control stalls
  receive a longer protocol allowance and GOAWAY with bounded drain.
- Native gRPC keepalive, first-request, idle, connection-age, and age-grace
  deadlines can be configured per listener. Connection age and idle retirement
  remain forceful with Tonic 0.14.6; active streams retain a finite age grace.
- The reference Worker's oversized Connect request response matches native
  Connect's 413 status and resource-exhausted error envelope.
- Route visibility requires the raw and decoded path to select the same rule:
  ordinary escaped characters are accepted, while encoded separators, percent
  signs, NUL, and dot segments receive a bad-request response.
- Over-cap sockets no longer pause the listener's shared accept loop. Trusted
  proxy client chains accept port forms, multiple field lines, and unspecified
  identities; malformed addresses are rejected. The nearest proxy field line
  controls the external host and scheme.
- FoundationDB delete attempts bounded metadata restoration after any failed
  phase. Operators can recreate missing metadata with `recover-delete` only
  when they supply a previously recorded tenant ID and the entire reserved
  metadata namespace is empty; application data may remain. Recovery records
  a new metadata creation time. Tenant creation and metadata initialization
  remain separate FoundationDB operations; an interrupted `ensure` requires
  explicit repair of an empty tenant.
  Application transactions opened with a read policy reject mutations.
- Explicit plaintext NATS connections ignore discovered servers. Native TLS
  clients use host certificate roots when available, with bundled public roots
  only as a fallback; HTTP clients offer HTTP/2 and HTTP/1.1 through ALPN.
- HTTP transport deadlines are configurable through validated server settings.
  JSONC rejects bare CR line endings to avoid ambiguous comment boundaries.
- Valkey TLS again installs the ring crypto provider when the host has not
  selected a process-wide provider, preserving the 0.3.1 default behavior.
- Service locators and Tailscale resolver configuration continue to require
  HTTPS; an unverified private transport cannot enable plain HTTP endpoints.
  Tailscale resolver suffixes also accept validated custom DNS domains.
- Preserve the 0.3.1 FoundationDB data-error enum and `clear` signatures while
  aborting any read-policy transaction that attempts a mutation, including
  when a callback ignores the method result.
- HTTP listeners answer liveness requests while application critical tasks are
  still becoming ready. App routes and `/metrics` may also respond during this
  interval; `/readyz` remains unavailable until startup completes. Overdue
  HTTP connection tasks are cancelled and joined before application cleanup
  starts. A saturated gRPC method responds with
  `RESOURCE_EXHAUSTED` instead of reaching the first-request transport deadline.
- WebSocket message handlers have a configurable bounded execution deadline
  that excludes bounded outbound socket writes. Browser Connect
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
