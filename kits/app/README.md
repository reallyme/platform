# reallyme-app-kit

App-kit provides host-neutral configuration, metadata, lifecycle contributions,
permissions, port descriptors, and metric naming. Use
[`apps/example`](../../apps/example/) for a working application and
[the architecture guide](../../docs/concepts/architecture.md) for crate boundaries.

## Configuration

The standard JSONC envelope accepts:

- `public_base_url` and `cors.allowed_origins`;
- `cookies.secure`, `cookies.domain`, and `cookies.same_site_policy`;
- `reflection_enabled`, which must be `false`;
- `downstream.<port_name>.base_url`, `endpoints`, `endpoint_selection`, and `locator`.

Supply custom fields through the application's typed configuration. Hosts load
and validate the document before passing it to the application. The reference
native server selects a checked-in profile with `application_profile`; it does
not parse `apps[].config_path` or `apps[].config_inline_jsonc`.

Each downstream target uses a singleton `base_url`, a static `endpoints` list,
or a host-resolved `locator`. Native hosts can resolve
`locator.mode = "tailscale_service"` through Tailscale Services/MagicDNS.
Configuration cannot enable gRPC/Connect schema introspection.

## Native HTTPS client

Enable `native-http` to use `BoundedHttpsClient`. Requests are constrained to one
validated HTTPS origin. Redirects, authority changes, ambiguous relative targets,
and requests or responses beyond the configured bounds are rejected. Encoded
request targets are limited to 8 KiB. Owned request copies and returned response
bytes use zeroizing buffers.

The default client uses operating-system certificate roots, falling back to
bundled public roots only if the host store is empty. For a private CA,
`BoundedHttpsClient::new_with_custom_tls_roots` accepts a nonempty
`rustls::RootCertStore` and trusts only those roots.

The caller supplies routes, authentication, response parsing, and retry policy.
Errors distinguish requests proven not dispatched from failures with an unknown
remote outcome. Retry an unknown outcome only when the operation has an
application-level idempotency guarantee.

## Metrics

Use fixed namespace/name pairs. The process accepts at most 1,024 distinct
pairs. Further new pairs increment the unlabelled
`reallyme_app_metric_series_rejected_total` counter; existing pairs continue to
work. Do not put user, request, or object identifiers in metric names.
