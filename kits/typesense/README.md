# reallyme-typesense-kit

Typesense connections, collection schemas, search/multi-search, and JSONL bulk
import. Construct a `TypesenseConnector` from validated configuration and use
its client for requests. Applications supply collection definitions and document
types. Tenant filters are never inserted automatically.

Responses are limited to 16 MiB after decompression. Split larger operations
into batches. Redirects are rejected. Timeouts apply to each attempt; include
retry attempts and backoff when setting an application deadline. Server retry
delays above the configured maximum return a rate-limit error without an early
retry. Jitter stays within that maximum.

The default connector uses operating-system certificate roots, falling back to
bundled public roots only when the host store is empty. For a private CA,
`TypesenseConnector::connect_with_custom_tls_roots` requires a nonempty
`rustls::RootCertStore` and HTTPS endpoints and trusts only those roots.
