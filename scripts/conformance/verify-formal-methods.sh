#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
model_dir="conformance/formal-methods"
path_policy="kits/server/src/http/layers/route_visibility/policy_path.rs"

: "${TLA2TOOLS_JAR:?Set TLA2TOOLS_JAR to the path to the TLA+ tools JAR}"
if [[ ! -f "$TLA2TOOLS_JAR" || ! -r "$TLA2TOOLS_JAR" ]]; then
  printf 'TLA2TOOLS_JAR must point to a readable file.\n' >&2
  exit 1
fi
jar_dir="$(cd "$(dirname "$TLA2TOOLS_JAR")" && pwd)"
jar_path="$jar_dir/$(basename "$TLA2TOOLS_JAR")"

# Kani writes intermediate files beside the harness. Keep those and TLC's
# state files outside the checkout while preserving the harness's source path.
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/$model_dir" "$scratch/$(dirname "$path_policy")"
cp "$repo_root/$path_policy" "$scratch/$path_policy"
cp "$repo_root/$model_dir/"{policy_path.rs,TenantDeletion.tla,TenantDeletion.cfg} "$scratch/$model_dir/"

cd "$scratch/$model_dir"
kani --output-format terse policy_path.rs
java -Xmx512m -XX:+UseParallelGC -cp "$jar_path" tlc2.TLC -workers 1 TenantDeletion
