#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe LLC
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

if ! command -v worker-build >/dev/null 2>&1; then
  printf 'worker-build 0.8.5 is required to validate the Worker bundle\n' >&2
  exit 1
fi

if ! rustup target list --installed | rg -qx 'wasm32-unknown-unknown'; then
  printf 'the wasm32-unknown-unknown Rust target is required to validate the Worker bundle\n' >&2
  exit 1
fi

pnpm --dir workers/example install --frozen-lockfile
pnpm --dir workers/example worker:check
