#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe LLC
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

readonly package="example-app"
readonly manifest="apps/example/Cargo.toml"
readonly features=(connect http native-server websocket connect-axum)

printf 'Checking %s without default features\n' "${package}"
cargo check -p "${package}" --no-default-features

for feature in "${features[@]}"; do
  if ! rg -q "^${feature} =" "${manifest}"; then
    printf 'example app manifest is missing required feature %s\n' "${feature}" >&2
    exit 1
  fi

  printf 'Checking %s feature %s\n' "${package}" "${feature}"
  cargo check -p "${package}" --no-default-features --features "${feature}"
done
