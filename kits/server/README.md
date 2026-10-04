# reallyme-server-kit

Native server runtime primitives for ReallyMe Platform. The kit owns listener
admission, HTTP and optional gRPC serving, shutdown and readiness, tracing,
metrics, and managed task supervision. Applications own their routes and
business behavior.

## Features

The default features enable HTTP, Connect, WebSocket support, and metrics.
`tonic-grpc` enables a separate native gRPC listener. The `testing` feature
adds server test helpers. Select features explicitly when a small dependency
graph or a transport-specific build is required.

The public facade crate has no default integrations. Its `native-server`
feature does not select Connect or WebSocket support; choose the corresponding
facade features explicitly.

## Listener security

Configure listener visibility, trusted proxy ranges, route visibility, body
limits, and rate-limit tiers before opening sockets. The connection limits
bound global and untrusted per-source occupancy. Trusted proxies and loopback
peers are exempt only from the per-source connection cap; the global cap still
applies. Request-level rate limits use validated forwarded client identity.

HTTP connection age, drain grace, idle, and stalled-write deadlines are
validated through `HttpTransportTimeouts` and set on `HttpServerConfig` with
`with_transport_timeouts`. Long-lived streams need explicit timeouts suited to
their workload. After a response body finishes, idle retirement allows at least
the stalled-write deadline for bytes queued to a slow reader. The server closes
excess sockets promptly and reports public HTTP errors in the stable JSON
envelope.

Native gRPC listeners accept validated `GrpcTransportTimeouts` through
`GrpcServerSpec::with_transport_timeouts`. The settings cover keepalive,
first-request, idle, age, and age-grace deadlines. Tonic 0.14.6 forcefully
retires a connection at the age-grace limit after an earlier GOAWAY; clients
should retry idempotent calls after transport failure. The correlation
interceptor replaces inbound request and trace IDs with server-generated values.

## Validation

Run the workspace formatting, check, test, doctest, and Clippy gates before
publishing. Single-feature builds should also be checked for `http`, `connect`,
`tonic-grpc`, and `testing`.
