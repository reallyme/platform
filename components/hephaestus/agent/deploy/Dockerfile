# SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
#
# SPDX-License-Identifier: MIT OR Apache-2.0

# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.98.1
FROM rust:1.98-bookworm@sha256:82150a52ec202c1b14d7817e14516c392bb7f5cfebd88f1ed531cb37ebd39922 AS builder
ARG RUST_VERSION

WORKDIR /workspace

RUN rustup toolchain install "${RUST_VERSION}" --profile minimal \
    && rustup default "${RUST_VERSION}"

COPY Cargo.toml Cargo.lock license.txt ./
COPY components/hephaestus/agent ./components/hephaestus/agent
COPY components/hephaestus/contract ./components/hephaestus/contract
COPY components/hephaestus/domain ./components/hephaestus/domain
COPY kits/reallyme-app-kit ./kits/reallyme-app-kit

# The full platform workspace includes sibling private repositories that are not
# part of this Docker build context. The agent image is intentionally built from
# the smallest workspace slice it needs, while still using the real checked-in
# workspace dependency versions and lockfile.
RUN perl -0pi -e 's#members = \[.*?\]#members = [\n    "components/hephaestus/contract",\n    "components/hephaestus/domain",\n    "components/hephaestus/agent",\n    "kits/reallyme-app-kit",\n]#s' Cargo.toml \
    && cargo build --release -p hephaestus-agent

FROM debian:bookworm-slim@sha256:67b30a61dc87758f0caf819646104f29ecbda97d920aaf5edc834128ac8493d3 AS runtime-base

RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates \
    && rm -rf /var/lib/apt/lists/*

USER root:root
WORKDIR /
STOPSIGNAL SIGTERM

ENTRYPOINT ["/usr/local/bin/hephaestus-agent"]
CMD ["--config", "/etc/reallyme/hephaestus-agent/config.toml"]

FROM runtime-base AS runtime

COPY --from=builder /workspace/target/release/hephaestus-agent /usr/local/bin/hephaestus-agent

RUN chmod 0555 /usr/local/bin/hephaestus-agent

FROM scratch AS artifact

COPY --from=builder /workspace/target/release/hephaestus-agent /hephaestus-agent

FROM runtime AS local-smoke

# Local smoke should be visible when run in the foreground. The production
# runtime target intentionally does not set a default filter; deployment config
# decides production verbosity.
ENV RUST_LOG=info

# Local smoke tests need the agent to exercise the registration/reporting path
# without requiring a live tailnet inside Docker. Production hosts get the real
# Tailscale package from the golden image; this target is intentionally selected
# only with `--target local-smoke`.
COPY components/hephaestus/agent/deploy/hephaestus-agent-local-tailscale /usr/bin/tailscale

RUN chmod 0555 /usr/bin/tailscale
