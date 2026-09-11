# reallyme-server-kit Examples

The examples in this directory show how deployable ReallyMe server processes should
compose server-kit primitives without adding product logic to the kit itself.

## `minimal_server.rs`

Demonstrates:

- typed server name validation
- validated HTTP and observability config construction
- tracing and Prometheus recorder initialization
- readiness state ownership
- operational route wiring for `/healthz`, `/readyz`, `/version`, and
  `/metrics`
- standard HTTP layers
- managed background task registration and graceful shutdown

Run locally with:

```text
cargo run --example minimal_server
```

The example does not bind a socket. Real server processes should map their own
environment variables into server-kit config types, bind listeners in their
binary crate, and keep product routes outside `reallyme-server-kit`.

## Trusted Proxy Metadata

`reallyme-server-kit` owns trusted proxy normalization because it must run
before app adapters observe requests.

Current behavior:

- proxy headers are ignored unless the direct peer socket IP is inside the
  configured trusted proxy ranges
- trusted peers may normalize external client IP, host, scheme, and explicit
  port
- host allowlisting evaluates the normalized external authority when trusted
  forwarded host metadata is enabled
- precedence is explicit: `Forwarded` first, then `X-Forwarded-*`, then direct
  request metadata
- conflicting `Forwarded` vs `X-Forwarded-*` host/proto metadata may fail
  closed when `strict_forwarded_header_consistency` is enabled
- malformed trusted forwarded host/proto metadata is rejected with a stable
  `400 Bad Request`
- raw proxy headers are stripped before app handlers run

When `require_https_external_scheme` is `true`:

- trusted proxied requests must prove `https` through trusted forwarded scheme
  metadata
- direct requests are allowed only for private or loopback operational health
  checks
- ordinary direct product requests fail closed

### Production Behind Cloudflare Or Vultr LB

```jsonc
"cors_allowed_origins": [
  "https://reallyme.net",
  "https://www.reallyme.net",
  "https://app.reallyme.net"
],
"security": {
  "allowed_hosts": [
    "api.reallyme.net",
    "reallyme.net",
    "www.reallyme.net"
  ],
  "trust_proxy_headers": true,
  "trusted_proxy_ranges": ["10.0.0.0/8"],
  "external_origin_policy": {
    "trusted_forwarded_host": true,
    "trusted_forwarded_proto": true,
    "require_https_external_scheme": true,
    "strict_forwarded_header_consistency": false,
    "strip_raw_proxy_headers": true
  },
  "operational_routes_public": false,
  "security_headers_enabled": true
},
"observability": {
  "request_log_mode": "errors_only",
  "request_log_fields": [
    "listener_name",
    "external_origin",
    "normalized_client_ip"
  ]
},
"rate_limit_tiers": {
  "public": {
    "refill_tokens_per_second": 25,
    "burst_tokens": 25,
    "max_distinct_sources": 10000,
    "scope": "per_source"
  }
}
```

This posture supports deployments where the ingress may supply:

- `Forwarded`
- `X-Forwarded-For`
- `X-Forwarded-Host`
- `X-Forwarded-Proto`

### Local Development

```jsonc
"cors_allowed_origins": ["http://localhost:3000"],
"security": {
  "allowed_hosts": ["127.0.0.1:8080", "localhost:8080"],
  "trust_proxy_headers": false,
  "operational_routes_public": false,
  "security_headers_enabled": true
}
```

Local development should usually leave `require_https_external_scheme` disabled
unless a trusted local reverse proxy is part of the workflow.

### Direct Internal Service

```jsonc
"cors_allowed_origins": [],
"security": {
  "allowed_hosts": ["api.internal.reallyme.net", "10.0.2.15:8080"],
  "trust_proxy_headers": false,
  "operational_routes_public": false,
  "security_headers_enabled": true
}
```

Direct internal services normally validate `Host` directly and do not depend on
forwarded metadata at all.

Normalized request extensions exposed to downstream handlers:

- `ForwardedClientIp`
- `ForwardedHost`
- `ForwardedProto`
- `ExternalRequestOrigin`

`strip_raw_proxy_headers` is intentionally fail-closed in this release: the
only supported production posture is `true`, so app code reads normalized
metadata instead of client-controlled header strings.

Conflicting lower-priority headers are never logged verbatim. When strict mode
is disabled, server-kit normalizes by precedence and emits only low-cardinality
debug reasons without raw forwarded header values.

## Config Ownership

Nearby configuration often sounds related, but not all of it belongs in
`server-kit`.

- `public_base_url`: app config, already supported through `reallyme-app-kit`
  standard config documents
- `allowed_hosts`: server JSONC, already supported in `server-kit`
- `trusted_proxy_ranges`: server JSONC, already supported in `server-kit`
- `max_request_body_bytes`: server JSONC today as `request_body_limit_bytes`
- `request_timeout_ms`: server JSONC today as `request_timeout_millis`
- `cors_allowed_origins`: now supported in server JSONC as an explicit
  listener/server-level list, with legacy single-origin compatibility for
  older configs
- `cookie_secure`: app config through `reallyme-app-kit`
- `cookie_domain`: app config through `reallyme-app-kit`
- `same_site_policy`: app config through `reallyme-app-kit`
- `rate_limit_policy`: supported in server JSONC through listener and
  route/service rate-limit tiers, plus explicit tier bucket `scope`
- `health_check_paths`: not currently configurable; server-kit keeps canonical
  fixed operational paths
- `admin/ops route exposure`: already supported through
  `operational_routes_public`, listener visibility, and route visibility policy
- `structured access log fields`: supported in server JSONC through bounded
  `request_log_fields`, using normalized request metadata only
- `external_origin_policy`: supported as an explicit nested HTTP security
  section so proxy trust policy can grow without flattening more top-level
  booleans into `security`
