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

Server-kit normalizes proxy metadata before app adapters receive a request. A
listener must configure trusted peer ranges and select one forwarded-header
family. The default family is `X-Forwarded-*`; `Forwarded` can be selected
explicitly. Headers from other families are stripped. Client IP selection
walks the selected chain from the trusted proxy side toward the client.

Host allowlisting and external HTTPS policy use the normalized authority and
scheme. If strict forwarded-header consistency is enabled, mixed header
families are rejected. App handlers receive normalized extensions and should
not read raw forwarding headers.

The reference server in `servers/example` accepts validated JSONC for
`allowed_hosts`, `trusted_proxy_ranges`, and `external_origin_policy` alongside
its profile, bind address, and bounded timeouts. A non-loopback bind requires a
nonempty host allowlist; an external-origin policy requires trusted proxy
ranges. Route visibility and rate-limit tiers remain host composition choices.

HTTP and gRPC listeners also accept validated `ConnectionLimitConfig` values
for total live TCP connections and per-source connections. The defaults are
2,048 and 64. A configured trusted proxy peer, or a loopback sidecar, uses only
the total limit so it can carry many client connections. The reference server
accepts an optional `connection_limits` JSONC object with `max_live` and
`max_per_source` fields. Request-level client policy should use the validated
forwarded client identity after proxy normalization.
When a listener reaches its total connection limit, it closes newly accepted
sockets promptly. Idle HTTP and gRPC connections close after 60 seconds;
stalled HTTP writes fail after 30 seconds. Active streams can finish during the
configured drain window. Rate-limit source tables replace fully refilled
sources when full. New sources otherwise share fixed overflow shards with a
bounded allowance; `reallyme_rate_limit_overflow_decisions_total` reports
admissions and rejections from that allowance.
Keep the sum of configured `max_distinct_sources` values at or below 25,000
per listener. The runtime warns on overcommit, and the shared registry cap
takes precedence when multiple tiers compete for source buckets.
Per-source tiers can set the IPv6 grouping prefix between `/48` and `/128`
through `HttpRateLimitTierPolicy::with_ipv6_source_prefix_len`; the default is `/64`.
