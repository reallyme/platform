# hephaestus-agent

`hephaestus-agent` is the node-local actuator for Hephaestus-managed VPS hosts.
It is intentionally not a scheduler and not a source of truth.

The central Hephaestus control plane owns desired state, ordinals, provider IDs,
Tailscale tags/services, image refs, execution locks, and topology decisions.
The agent reports local state and performs bounded node-local actions only.

## Network Model

The agent is outbound-first. It talks to the Hephaestus control plane through
the generated Connect client in `reallyme-hephaestus-contract`.

Production hosts should point at the Tailscale service endpoint:

```toml
controller_base_url = "https://heph.example.ts.net"
```

Plain HTTP is allowed only for private controller endpoints: local addresses,
RFC1918 private addresses, Tailscale `100.64.0.0/10` addresses, or Tailscale
MagicDNS names under `.ts.net`. Arbitrary public `http://` controller URLs are
rejected. This keeps first-boot registration simple on the tailnet while still
preventing accidental plaintext control-plane traffic over the public internet.

## Local Inputs

The golden image should install the binary and systemd unit, but should not
contain node identity, Tailscale identity, registry credentials, service secrets,
or service data.

The production unit is installed at `/etc/systemd/system/hephaestus-agent.service`
and the binary at `/usr/local/bin/hephaestus-agent`. The unit is intentionally
disabled in the golden image: cloud-init must write config, bootstrap material,
and Tailscale identity first, then enable and start the service. The unit runs as
root because it must use the Docker socket, systemd, and Tailscale CLI, but it is
hardened with `ProtectSystem=strict`, `NoNewPrivileges`, private devices/tmp,
restricted address families, an empty capability bounding set, and explicit
ReallyMe-only write paths.

Cloud-init writes `/etc/reallyme/hephaestus-agent/config.toml` and a short-lived
bootstrap token at `/etc/reallyme/hephaestus-agent/bootstrap-token`.

On first boot, the agent creates or loads a node-local Ed25519 identity key
under `/var/lib/reallyme/hephaestus-agent/`. Golden images must not contain this
file. The agent calls the generated `RegisterAgent` Connect RPC with its typed
boot report, one-time bootstrap token, public key, and public-key fingerprint.
If Hephaestus accepts the registration, the response includes a node-local
runtime token and desired-state generation. The server derives that token from
the node identity, including the public-key fingerprint supplied during
registration. The agent writes the runtime token and observed generation under
`/var/lib/reallyme/hephaestus-agent/` with restrictive permissions, deletes the
bootstrap token, and uses `Authorization: Bearer <runtime-token>` for later
`SubmitAgentReport`, `PollAgentActions`, and `CompleteAgentAction` RPCs.

### Re-enrollment

If the runtime token is deleted or corrupt after the bootstrap token has already
been deleted, the agent must not silently create a new identity or self-enroll.
It logs `BootstrapTokenUnavailable` and exits so systemd makes the failure
visible. Recovery is a central control-plane action: Hephaestus must reissue
fresh bootstrap material for that node identity, or the node must be rebuilt from
a clean golden image.

## Observation Sources

- Docker Engine API over `/var/run/docker.sock` for container state, image pull,
  stop/restart, and Docker Engine version.
- Docker Compose CLI only for applying a deterministic rendered service
  allocation and reading the Compose plugin version.
- systemd, invoked without a shell and only for configured unit names.
- cAdvisor metrics from a local URL, normally `http://127.0.0.1:8080/metrics`.
- node-exporter metrics from a local URL, normally
  `http://127.0.0.1:9100/metrics`.
- Local HTTP observation and health-probe URLs must use literal loopback
  addresses such as `127.0.0.1` or `::1`; `localhost` is intentionally rejected
  to avoid resolver or `/etc/hosts` ambiguity.
- Tailscale CLI, invoked without a shell for IPs, hostname, MagicDNS, tags, and
  advertised service-state checks.
- Local HTTP/TCP service probes declared in config.
- `/proc` for host CPU/memory/boot data.
- `/var/run/reboot-required` and `unattended-upgrades.service` for patch and
  reboot posture.

The agent does not scrape or ship application logs by default because logs may
contain user data.

## Logging

`hephaestus-agent` uses JSON log format unconditionally, which is ideal for
`journald` ingestion and machine-readable alerting.

For local troubleshooting, filter and pretty-print with:

```sh
journalctl -u hephaestus-agent -n 200 | jq -r '.'
```

## Control Actions

The agent polls Hephaestus with `PollAgentActions` and completes each accepted
action with `CompleteAgentAction`. Actions are generated protobuf messages, not
JSON commands or shell snippets.

Supported action kinds:

- `REPORT_NOW`
- `PULL_IMAGE`
- `CONFIGURE_DOCKER_SERVICE`
- `RESTART_CONTAINER`
- `STOP_CONTAINER`
- `START_SERVICE`
- `STOP_SERVICE`
- `RESTART_SERVICE`
- `DRAIN_TAILSCALE_SERVICE`
- `ADVERTISE_TAILSCALE_SERVICE`
- `REBOOT_HOST`

Every action must include a matching node id, idempotency key, desired-state
generation, and deadline. The agent rejects expired, unknown, malformed, or
wrong-node actions. Local execution uses fixed binaries and validated arguments;
there is no arbitrary command execution path.

Every polled action is also written to the local durable audit log at
`/var/lib/reallyme/hephaestus-agent/action-audit.jsonl` by default. The audit log
records received, accepted, started, completed, rejected, and remote completion
reporting events. Records are JSONL with `0600` permissions and contain only
redacted identifiers, action kind, generation, status, and reason; action
payloads, environment values, rendered secrets, and token material are never
serialized.

`CONFIGURE_DOCKER_SERVICE` is the fast redeploy path for prepared hosts. The
payload is a typed allocation spec: service id, container name, image ref and
optional digest, environment bindings, private port mappings, constrained
volumes, rendered config files, runtime secret files, local health probes,
restart policy, and Tailscale Services. Immediately before execution, the agent
uses `DockerRuntimeAuthorityService` to resolve exceptional host authority for
the validated action identity and service. The authority contract currently
permits only the TUN device and the `NET_ADMIN` and `NET_BIND_SERVICE` Linux
capabilities. Unknown, unspecified, duplicate, or inconsistent values are
rejected; TUN access requires `NET_ADMIN`. The agent renders explicit `devices`
and `cap_add` entries with `cap_drop: ALL`, never `privileged: true`. Authority
also selects a read-only root filesystem, `no-new-privileges`, and a typed
bridge or host network mode without relying on a service-name convention. An
older controller that does not implement the authority service grants no new
device or capability access, preserving safe rolling upgrades. The agent renders
`/etc/reallyme/services/{service}/compose.yml` with a Compose `environment:`
block so YAML serialization, not a hand-written `.env` parser, owns value
escaping. Rendered config files live under
`/etc/reallyme/services/{service}/config/`; runtime secrets are written under
`/etc/reallyme/secrets/{service}/` with `0600` permissions. Those config and
secret directories are mounted read-only into the workload at `/config` and
`/run/secrets/reallyme`. The agent writes only under approved ReallyMe service
roots, runs `docker compose` with fixed arguments, health-gates the workload,
then renders `/etc/reallyme/tailscale-services/serveconfig.json`, applies it
with `tailscale serve set-config --all`, and advertises only the approved
service names.

Secret values are not carried in `CONFIGURE_DOCKER_SERVICE` payloads. Each
rendered secret file declares only an opaque `secret_ref`; the agent issues a
`ResolveAgentSecret` Connect RPC scoped to
`(node_id, action_id, idempotency_key, secret_ref)`, then writes the resolved
bytes to disk with `0600` permissions immediately before applying the new
Compose allocation. This keeps durable server-side action queues and the local
audit log free of credential material.

Docker service deploys are health-gated and rollback-aware. Before rendering a
new allocation, the agent snapshots the previous rendered service and secret
files into a service-local rollback area under `/var/lib/reallyme/{service}/`.
If image pull, digest verification, container start, or local health probes fail,
the agent stops the failed Compose allocation, restores the previous rendered
files, and brings the previous Compose allocation back up. Digest-pinned images
are inspected after pull and rejected if Docker cannot prove the expected
`sha256:` digest is present locally.

Drain actions call `tailscale serve drain` and remove that specific service from
the rendered serve config before applying the updated config. Advertisement
actions advertise an already-rendered service definition; service endpoint
rendering is owned by `CONFIGURE_DOCKER_SERVICE`.

The central Hephaestus service currently exposes the Connect RPC surface and
validates completions fail-closed. Durable server-side action allocation should
be backed by the operational-state database before dashboard buttons enqueue
live host mutations.
