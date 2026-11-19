# reallyme-server-kit

Native HTTP, Connect RPC, optional gRPC, readiness, shutdown, tracing, metrics,
and managed tasks. Run the [example server](../../servers/example/) for a
listener-backed application.

Default features enable HTTP, Connect, WebSockets, and metrics. `tonic-grpc`
adds a separate gRPC listener; `testing` adds test helpers. The facade's
`native-server` feature does not enable Connect or WebSockets: select those
facade features separately.

## Listener configuration

Set allowed hosts, trusted proxy ranges, route visibility, body limits, and
rate limits before accepting external traffic. Non-loopback binds in the
reference server require a host allowlist. Its JSONC accepts `allowed_hosts`,
`trusted_proxy_ranges`, `external_origin_policy`, and
`connection_limits` with `max_live` and `max_per_source`.

`ConnectionLimitConfig` defaults to 2,048 live TCP connections and 64 per source.
Trusted proxies and loopback peers are exempt from the per-source cap, but not
from the global cap. Excess sockets are closed.

Select one forwarded-header family: `X-Forwarded-*` by default, or `Forwarded`.
Configure trusted peer ranges before enabling forwarded host or scheme handling.
Use normalized request extensions in app handlers; raw forwarded headers are
not a trusted client identity. Strict consistency mode rejects mixed families.

Keep the sum of rate-limit tier `max_distinct_sources` at or below 25,000 per
listener. Overcommitted tiers share that fixed capacity; excess identities use
bounded overflow allowances. The `reallyme_rate_limit_overflow_decisions_total`
counter records allowed and rejected overflow decisions. IPv6 grouping defaults
to `/64`; `with_ipv6_source_prefix_len` accepts `/48` through `/128`.

## Timeouts and readiness

Use `HttpServerConfig::with_transport_timeouts` with validated
`HttpTransportTimeouts`. Defaults include 60-second idle and 30-second
stalled-write limits. Configure connection age and drain grace for long-lived
streams. After a response body finishes, idle retirement allows at least the
stalled-write deadline to flush queued bytes.

For gRPC, `GrpcServerSpec::with_transport_timeouts` accepts keepalive,
first-request, idle, age, and age-grace limits. The age-grace deadline can
terminate unfinished calls. Retry only operations that are safe to replay.
Read request and trace IDs from server responses; the listener replaces inbound
correlation IDs.

Route traffic using `/readyz`. HTTP listeners can respond while startup checks
are still running, and readiness is withdrawn before shutdown drains requests.
HTTP/3 is unsupported. Hosts terminate TLS outside the native HTTP listener.

## Examples and tests

The composition-only example constructs routes and shuts down its task; it does
not bind a socket:

```sh
cargo run -p reallyme-server-kit --example minimal_server
scripts/conformance/verify-server-kit-feature-separation.sh
```

See [Building and testing](../../docs/guides/validation.md) for workspace checks.
