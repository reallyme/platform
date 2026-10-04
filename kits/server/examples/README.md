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
