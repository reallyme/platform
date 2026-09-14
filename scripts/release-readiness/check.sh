#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe LLC
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

contracts_are_generated=false
if [[ "$#" -eq 1 && "$1" == --contracts-generated ]]; then
  contracts_are_generated=true
elif [[ "$#" -ne 0 ]]; then
  printf 'usage: %s [--contracts-generated]\n' "$0" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

node scripts/check_release_readiness.mjs
cargo fmt --all -- --check
if [[ "${contracts_are_generated}" != true ]]; then
  scripts/generation/generate-rust-contracts.sh
fi
generated_status="$(git status --porcelain=v1 --untracked-files=all -- \
  apps/example/contract/src/generated)"
if [[ -n "${generated_status}" ]]; then
  printf '%s\n' "${generated_status}" >&2
  printf 'committed generated contract sources are stale; regenerate and commit the result\n' >&2
  exit 1
fi
scripts/policy/verify-contract-boundaries.sh
scripts/policy/verify-crypto-policy.sh
scripts/conformance/verify-example-app-feature-separation.sh
scripts/conformance/verify-server-kit-feature-separation.sh
scripts/policy/verify-workspace-standards.sh
scripts/build/check-worker.sh
node --test scripts/release/*.test.mjs
node scripts/release/publish_crates_in_order.mjs order
cargo check --locked -p reallyme-platform --no-default-features
cargo check --locked --workspace --all-features
cargo test --locked --workspace --all-features
cargo test --locked --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings

if command -v cargo-deny >/dev/null 2>&1; then
  cargo deny check
else
  printf 'SKIP cargo deny check: cargo-deny is not installed\n' >&2
fi

if command -v cargo-audit >/dev/null 2>&1; then
  cargo audit
else
  printf 'SKIP cargo audit: cargo-audit is not installed\n' >&2
fi

if command -v cargo-machete >/dev/null 2>&1; then
  cargo machete
else
  printf 'SKIP cargo machete: cargo-machete is not installed\n' >&2
fi
