#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
model_dir="conformance/formal-methods"
path_policy="kits/server/src/http/layers/route_visibility/policy_path.rs"
route_admission="kits/server/src/http/visibility/admission.rs"
route_selection="kits/server/src/http/visibility/selection.rs"

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
mkdir -p "$scratch/$model_dir"
for source in "$path_policy" "$route_admission" "$route_selection"; do
  mkdir -p "$scratch/$(dirname "$source")"
  cp "$repo_root/$source" "$scratch/$source"
done
cp "$repo_root/$model_dir/"{policy_path.rs,route_admission.rs,route_selection.rs} "$scratch/$model_dir/"
cp "$repo_root/$model_dir/"{TenantDeletion.tla,TenantDeletion.cfg} "$scratch/$model_dir/"
cp "$repo_root/$model_dir/"{TenantRecovery.tla,TenantRecovery.cfg,TenantRecoveryLookupOnly.cfg} \
  "$scratch/$model_dir/"
cp "$repo_root/$model_dir/"{TenantRestoration.tla,TenantRestoration.cfg} "$scratch/$model_dir/"
cp "$repo_root/$model_dir/"{TenantRestorationByName.cfg,TenantRestorationSkipWrite.cfg} \
  "$scratch/$model_dir/"

cd "$scratch/$model_dir"
kani --output-format terse policy_path.rs
kani --output-format terse route_admission.rs
kani --output-format terse route_selection.rs

run_tlc() {
  java -Xmx512m -XX:+UseParallelGC -cp "$jar_path" tlc2.TLC -workers 1 "$@"
}

expect_invariant_violation() {
  local module="$1" config="$2" invariant="$3"
  local log="$scratch/$config.log" result=0

  # Require both the safety-violation exit code and the named invariant.
  # Parser and tool failures must never satisfy a counterexample check.
  run_tlc -config "$config.cfg" "$module" >"$log" 2>&1 || result=$?
  cat "$log"
  if [[ "$result" -ne 12 ]] ||
    ! grep -Fxq "Error: Invariant $invariant is violated." "$log"; then
    printf '%s must violate %s.\n' "$config" "$invariant" >&2
    return 1
  fi
}

run_tlc TenantDeletion
run_tlc TenantRecovery
expect_invariant_violation TenantRecovery TenantRecoveryLookupOnly RecoveryWritesOnlyRecordedTenant
run_tlc TenantRestoration
expect_invariant_violation TenantRestoration TenantRestorationByName RestoreWritesOnlyOriginal
expect_invariant_violation TenantRestoration TenantRestorationSkipWrite AcknowledgedRestoreSurvives
