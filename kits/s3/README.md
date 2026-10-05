# S3-compatible storage kit

Reusable endpoint-configurable S3 object storage primitives for ReallyMe services.

This kit owns:

- HTTPS endpoint and credential validation
- deterministic object-key validation
- AWS SigV4 signing for bounded uploads
- a small reusable upload client

This kit does not own:

- app-specific object naming policy
- app-specific serialization
- service runtime wiring
- dashboard or API contracts

Uploads are limited to 64 MiB. The native client requires HTTPS, rejects redirects,
and uses a 10-second connection timeout and a 60-second request timeout. It leaves
object bytes unmodified by automatic response decompression. The Worker client
also rejects redirects. Use the final storage endpoint when configuring either
client; signed requests cannot be redirected to a different authority.

`S3StorageClient::new` uses operating-system certificate roots, with bundled
public roots only when the host store is empty. For a private CA, pass a
nonempty `rustls::RootCertStore` to
`S3StorageClient::new_with_custom_tls_roots`. That client trusts only the
supplied roots.

Conditional uploads report an existing object only for HTTP 412. HTTP 409 is a
conflict failure that callers may retry according to their own bounded policy.
