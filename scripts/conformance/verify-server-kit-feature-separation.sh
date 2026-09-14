#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe LLC
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

features=(http connect websocket metrics testing tonic-grpc)

printf 'Checking reallyme-server-kit without default features\n'
cargo check -p reallyme-server-kit --no-default-features

for feature in "${features[@]}"; do
  printf 'Checking reallyme-server-kit feature %s\n' "${feature}"
  cargo check -p reallyme-server-kit --no-default-features --features "${feature}"
done
