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

# worker-build downloads pinned wasm-bindgen and wasm-opt release artifacts on a
# cold machine. GitHub occasionally returns a transient gateway error for those
# assets, so retry only that fetch failure; compilation and bundle errors remain
# immediate failures and retain their original exit status.
readonly worker_download_attempts=5
readonly worker_download_retry_delay_seconds=2
worker_check_log="$(mktemp "${TMPDIR:-/tmp}/reallyme-worker-check.XXXXXX")"
trap 'rm -f "${worker_check_log}"' EXIT

for ((attempt = 1; attempt <= worker_download_attempts; attempt += 1)); do
  set +e
  pnpm --dir workers/example worker:check 2>&1 | tee "${worker_check_log}"
  worker_check_status="${PIPESTATUS[0]}"
  set -e

  if ((worker_check_status == 0)); then
    exit 0
  fi

  if ! rg -q 'Failed to fetch URL' "${worker_check_log}" ||
    ((attempt == worker_download_attempts)); then
    exit "${worker_check_status}"
  fi

  printf 'Worker tool download failed; retrying (%d/%d)\n' \
    "$((attempt + 1))" "${worker_download_attempts}" >&2
  sleep "${worker_download_retry_delay_seconds}"
  : >"${worker_check_log}"
done

exit 1
