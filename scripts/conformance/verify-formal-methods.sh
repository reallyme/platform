#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

mode="${1:-all}"
if [[ $# -gt 1 || ( "$mode" != all && "$mode" != kani && "$mode" != tlc ) ]]; then
  printf 'Usage: %s [all|kani|tlc]\n' "$0" >&2
  exit 2
fi

timed_check() {
  local label="$1" started="$SECONDS" result=0
  shift
  printf 'Running %s\n' "$label"
  "$@" || result=$?
  printf '%s finished in %ss (exit %s)\n' "$label" "$((SECONDS - started))" "$result"
  return "$result"
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
model_dir="conformance/formal-methods"
path_policy="kits/server/src/http/layers/route_visibility/policy_path.rs"
route_admission="kits/server/src/http/visibility/admission.rs"
route_selection="kits/server/src/http/visibility/selection.rs"
proxy_trust="kits/server/src/http/layers/security/proxy_trust.rs"
rate_limit_refill="kits/server/src/runtime/rate_limit/refill.rs"
grpc_authorization="kits/server/src/grpc/authorize_call.rs"
tenant_keyspace="kits/foundationdb/src/fdb/tenant/data_keyspace.rs"
grpc_deadline="kits/server/src/grpc/deadline_wire.rs"
fdb_retry_budget="kits/foundationdb/src/fdb/tenant/transact_budget.rs"

if [[ "$mode" != kani ]]; then
  : "${TLA2TOOLS_JAR:?Set TLA2TOOLS_JAR to the path to the TLA+ tools JAR}"
  if [[ ! -f "$TLA2TOOLS_JAR" || ! -r "$TLA2TOOLS_JAR" ]]; then
    printf 'TLA2TOOLS_JAR must point to a readable file.\n' >&2
    exit 1
  fi
  jar_dir="$(cd "$(dirname "$TLA2TOOLS_JAR")" && pwd)"
  jar_path="$jar_dir/$(basename "$TLA2TOOLS_JAR")"
fi

# Kani writes intermediate files beside the harness. Keep those and TLC's
# state files outside the checkout while preserving the harness's source path.
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/$model_dir"
if [[ "$mode" != tlc ]]; then
  for source in "$path_policy" "$route_admission" "$route_selection" "$proxy_trust" \
    "$rate_limit_refill" "$grpc_authorization" "$tenant_keyspace" "$grpc_deadline" "$fdb_retry_budget"; do
    mkdir -p "$scratch/$(dirname "$source")"
    cp "$repo_root/$source" "$scratch/$source"
  done
  kani_harnesses=(
    policy_path_tests.rs route_admission_tests.rs route_selection_tests.rs proxy_trust_tests.rs
    rate_limit_refill_tests.rs grpc_authorization_tests.rs tenant_keyspace_tests.rs
    grpc_deadline_tests.rs fdb_retry_budget_tests.rs
  )
  for harness in "${kani_harnesses[@]}"; do
    cp "$repo_root/$model_dir/$harness" "$scratch/$model_dir/$harness"
  done
fi
if [[ "$mode" != kani ]]; then
  cp "$repo_root/$model_dir/"{TenantDeletion.tla,TenantDeletion.cfg} "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"{TenantRecovery.tla,TenantRecovery.cfg,TenantRecoveryLookupOnly.cfg} \
    "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"{TenantRestoration.tla,TenantRestoration.cfg} "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"{TenantRestorationByName.cfg,TenantRestorationSkipWrite.cfg} \
    "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"{TenantMapDeletion.tla,TenantMapDeletion.cfg,TenantMapDeletionSkipRetryId.cfg} \
    "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"{TenantDeleteRecovery.tla,TenantDeleteRecovery.cfg} "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"{TenantDeleteRecoveryExhaustion.cfg,TenantDeleteRecoveryLateDelete.cfg} \
    "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"{TenantDeletionRecord.tla,TenantDeletionRecord.cfg} "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"{TenantDeletionRecordLookupOnly.cfg,TenantDeletionRecordSkipFence.cfg} \
    "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"{TenantReservation.tla,TenantReservation.cfg} "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"{TenantReservationPreflightOnly.cfg,TenantReservationCrashRecovery.cfg} \
    "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"TenantReservationPruning{.tla,.cfg,Ambiguous.cfg,NameReuse.cfg,Replay.cfg} \
    "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"TenantReservationCutoff{.tla,.cfg,Preflight.cfg,EarlyReclaim.cfg,Progress.cfg} \
    "$scratch/$model_dir/"
  cp "$repo_root/$model_dir/"ServerLifecycle{.tla,.cfg,SignalError.cfg,EarlyDrain.cfg,Progress.cfg} \
    "$scratch/$model_dir/"

  cp "$repo_root/$model_dir/"FdbTransactionDeadline{.tla,.cfg,LateRetry.cfg,LateSubmit.cfg,ReplayWrite.cfg,LateCommit.cfg} \
    "$scratch/$model_dir/"
fi

cd "$scratch/$model_dir"
if [[ "$mode" != tlc ]]; then
  for harness in "${kani_harnesses[@]}"; do
    timed_check "Kani $harness" kani --output-format terse "$harness"
  done
fi

run_tlc() {
  timed_check "TLC $*" java -Xmx512m -XX:+UseParallelGC -cp "$jar_path" tlc2.TLC -workers 1 "$@"
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

if [[ "$mode" != kani ]]; then
  run_tlc TenantDeletion
  run_tlc TenantRecovery
  expect_invariant_violation TenantRecovery TenantRecoveryLookupOnly RecoveryWritesOnlyRecordedTenant
  run_tlc TenantRestoration
  expect_invariant_violation TenantRestoration TenantRestorationByName RestoreWritesOnlyOriginal
  expect_invariant_violation TenantRestoration TenantRestorationSkipWrite AcknowledgedRestoreSurvives
  run_tlc TenantMapDeletion
  expect_invariant_violation TenantMapDeletion TenantMapDeletionSkipRetryId ReplacementSurvives
  run_tlc TenantDeleteRecovery
  expect_invariant_violation TenantDeleteRecovery TenantDeleteRecoveryExhaustion ExhaustionPreservesMetadata
  expect_invariant_violation TenantDeleteRecovery TenantDeleteRecoveryLateDelete NoDeleteCommitAfterExhaustion
  run_tlc TenantDeletionRecord
  expect_invariant_violation TenantDeletionRecord TenantDeletionRecordLookupOnly RepairTargetsRecordedId
  expect_invariant_violation TenantDeletionRecord TenantDeletionRecordSkipFence SafeMetadataRecordIsAccurate
  run_tlc TenantReservation
  expect_invariant_violation TenantReservation TenantReservationPreflightOnly StaleTransactionsCannotCommit
  expect_invariant_violation TenantReservation TenantReservationCrashRecovery NoCrossHostCrashRecovery
  run_tlc TenantReservationPruning
  expect_invariant_violation TenantReservationPruning TenantReservationPruningAmbiguous NoReacquisitionAfterAmbiguousCommits
  expect_invariant_violation TenantReservationPruning TenantReservationPruningNameReuse NoReacquisitionAfterNameReuse
  expect_invariant_violation TenantReservationPruning TenantReservationPruningReplay NoRetiredReplayDuringNewReservation
  run_tlc TenantReservationCutoff
  expect_invariant_violation TenantReservationCutoff TenantReservationCutoffPreflight NoStaleClaimAfterNameReuse
  expect_invariant_violation TenantReservationCutoff TenantReservationCutoffEarlyReclaim NoSameGenerationClaimAfterReclaim
  expect_invariant_violation TenantReservationCutoff TenantReservationCutoffProgress NoProgressAfterCutoff
  run_tlc ServerLifecycle
  expect_invariant_violation ServerLifecycle ServerLifecycleSignalError ReadinessWithdrawnDuringShutdown
  expect_invariant_violation ServerLifecycle ServerLifecycleEarlyDrain ServingDrainWaitsForPropagation
  expect_invariant_violation ServerLifecycle ServerLifecycleProgress NoOrderedSignalErrorShutdown
  run_tlc FdbTransactionDeadline
  expect_invariant_violation FdbTransactionDeadline FdbTransactionDeadlineLateRetry NoAttemptAfterDeadline
  expect_invariant_violation FdbTransactionDeadline FdbTransactionDeadlineLateSubmit NoCommitSubmittedAfterDeadline
  expect_invariant_violation FdbTransactionDeadline FdbTransactionDeadlineReplayWrite NoDuplicateWrite
  expect_invariant_violation FdbTransactionDeadline FdbTransactionDeadlineLateCommit NoLateSettlement
fi
