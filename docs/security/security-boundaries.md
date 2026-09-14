# Security Boundaries

Platform treats every transport, configuration source, generated DTO, and
infrastructure response as an external boundary. Adapters validate and classify
those values before application core code receives them.

The application core must remain independent of listener ownership, process
lifecycle, transport frameworks, and host runtimes. This separation limits the
trusted computing base of business logic and makes host-specific hardening
auditable at a single boundary.

Secrets and owned PII must use secrecy and zeroization types where deterministic
erasure is possible. Errors and telemetry must classify failures without
carrying raw input, credentials, request bodies, or high-cardinality values.

Resource use is part of the security model. Request bodies, frames, queues,
retries, concurrency, retained state, and externally influenced allocations
must be explicitly bounded. Buffer and offset arithmetic uses checked
operations, and sensitive length conversions use `TryFrom` with typed failures.

Repository policy is enforced by
`scripts/policy/verify-workspace-standards.sh` and
`scripts/policy/verify-crypto-policy.sh`. The complete audit-facing validation
sequence is documented in `docs/guides/validation.md`.
