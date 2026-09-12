#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
#
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

contracts_are_generated=false
if [[ "$#" -eq 1 && "$1" == --contracts-generated ]]; then
  contracts_are_generated=true
elif [[ "$#" -ne 0 ]]; then
  printf 'usage: %s [--contracts-generated]\n' "$0" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

cargo fmt --all -- --check
if [[ "${contracts_are_generated}" != true ]]; then
  scripts/generate-rust-contracts.sh
fi
generated_status="$(git status --porcelain=v1 --untracked-files=all -- \
  apps/example/contract/src/generated \
  components/hephaestus/contract/src/generated)"
if [[ -n "${generated_status}" ]]; then
  printf '%s\n' "${generated_status}" >&2
  printf 'committed generated contract sources are stale; regenerate and commit the result\n' >&2
  exit 1
fi
scripts/verify-contract-boundaries.sh
scripts/verify-crypto-policy.sh
scripts/verify-example-app-feature-separation.sh
scripts/verify-server-kit-feature-separation.sh
scripts/verify-workspace-standards.sh
node --test scripts/*.test.mjs
node scripts/publish_crates_in_order.mjs order
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
