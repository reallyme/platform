# reallyme-typesense-kit

First-class Typesense connector and protocol primitives for native ReallyMe
services.

This kit owns the infrastructure boundary shared by every Typesense-backed app:

- validated HTTP/HTTPS endpoints and secret API-key configuration;
- pooled HTTP connector construction;
- reusable readiness checks against Typesense's health endpoint;
- bounded request deadlines, retry backoff, jitter, and endpoint failover;
- typed collection schemas and lifecycle operations;
- typed search and multi-search requests;
- bounded pagination, filters, field names, and sorting;
- typed JSONL bulk import results; and
- low-cardinality errors and metrics that do not retain documents or API keys.

Apps own their index schemas, document semantics, ranking policy, and conversion
to public response DTOs. Tenant filters are app policy and are never inserted
implicitly by this kit. Applications import this kit directly at their
infrastructure adapter boundary.

Responses are limited to 16 MiB after decompression, including search results and
JSONL import results. Split larger operations into bounded batches. HTTP redirects
are rejected to keep API keys confined to the configured endpoint. Server-provided
retry delays beyond the configured maximum return the rate-limit error without
retrying early. Retry jitter stays within that maximum.
