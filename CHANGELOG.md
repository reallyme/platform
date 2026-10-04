# Changelog

## 0.3.1 (prepared)

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
- Several server-kit APIs changed or were removed, including the shape of `CorsConfig`.
  Downstream integrations should compile against 0.3 before deployment.
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
