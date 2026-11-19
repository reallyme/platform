# reallyme-s3-kit

S3-compatible object uploads and AWS Signature Version 4 signing. Configure an
HTTPS endpoint, region, bucket, credentials, and key prefix through
`S3StorageConfig`. Credentials use `secrecy::SecretString`; temporary credentials
also require `with_session_token`.

Select `native-client` for `S3StorageClient` or `worker-client` for
`WorkerS3StorageClient`. Applications supply object naming and serialization.

Uploads are limited to 64 MiB. The native client requires HTTPS, rejects
redirects, and uses a 10-second connection timeout and a 60-second request
timeout. Automatic response decompression is disabled. The Worker client also
rejects redirects; configure the final storage endpoint for signed requests.

`S3StorageClient::new` uses operating-system certificate roots, falling back to
bundled public roots only when the host store is empty. For a private CA,
`S3StorageClient::new_with_custom_tls_roots` requires a nonempty
`rustls::RootCertStore` and trusts only those roots.

Conditional uploads report an existing object only for HTTP 412. HTTP 409 is a
conflict failure; callers decide whether their operation can be retried.
