------------------------- MODULE TenantDeletionRecord -------------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

EXTENDS Naturals

CONSTANTS OriginalId, ReplacementId, NoTenant, MaxRepairs, VerifyRepairId, FenceCompatibleMetadata

ASSUME /\ OriginalId # ReplacementId
       /\ NoTenant \notin {OriginalId, ReplacementId}
       /\ MaxRepairs \in Nat
       /\ MaxRepairs > 0
       /\ VerifyRepairId \in BOOLEAN
       /\ FenceCompatibleMetadata \in BOOLEAN

VARIABLES record, tenant, clear, deletion, repairs, worker

vars == <<record, tenant, clear, deletion, repairs, worker>>
TenantIds == {OriginalId, ReplacementId}
MappingIds == TenantIds \cup {NoTenant}
Metadata == {"empty", "saved", "repaired", "incompatible"}
Compatible == {"saved", "repaired"}
RecordStates == {"absent", "pending", "original-absent", "metadata-safe"}
CommitStates == {"idle", "submitted", "committed", "aborted"}
WorkerPhases == {"prepare", "clear", "delete", "offline", "lookup", "repair", "await-repair", "done"}
RepairSlots == 1..MaxRepairs
IdleRepair == [target |-> NoTenant, read |-> "empty", state |-> "idle"]
WorkerAt(phase) == [phase |-> phase, slot |-> 0]

Init ==
    /\ record = [id |-> NoTenant, state |-> "absent"]
    /\ tenant = [id |-> OriginalId, metadata |-> "saved", hasData |-> FALSE, reused |-> FALSE]
    /\ clear = "idle"
    /\ deletion = "idle"
    /\ repairs = [i \in RepairSlots |-> IdleRepair]
    /\ worker = WorkerAt("prepare")

\* The durable write must complete before any destructive transaction can
\* be submitted. A restart keeps the ID and never recreates a pending intent.
PersistIntent ==
    /\ worker.phase = "prepare"
    /\ record.state = "absent"
    /\ tenant.id = OriginalId
    /\ record' = [id |-> tenant.id, state |-> "pending"]
    /\ worker' = [worker EXCEPT !.phase = "clear"]
    /\ UNCHANGED <<tenant, clear, deletion, repairs>>

SubmitClear ==
    /\ worker.phase = "clear"
    /\ record.state = "pending"
    /\ clear = "idle"
    /\ tenant.id = record.id
    /\ ~tenant.hasData
    /\ clear' = "submitted"
    /\ UNCHANGED <<record, tenant, deletion, repairs, worker>>

CommitClear ==
    /\ clear = "submitted"
    /\ tenant.id = record.id
    /\ ~tenant.hasData
    /\ clear' = "committed"
    /\ tenant' = [tenant EXCEPT !.metadata = "empty"]
    /\ UNCHANGED <<record, deletion, repairs, worker>>

AbortClear ==
    /\ clear = "submitted"
    /\ clear' = "aborted"
    /\ UNCHANGED <<record, tenant, deletion, repairs, worker>>

ObserveClear ==
    /\ worker.phase = "clear"
    /\ clear = "committed"
    /\ worker' = [worker EXCEPT !.phase = "delete"]
    /\ UNCHANGED <<record, tenant, clear, deletion, repairs>>

\* This step abstracts the transactional ID read and clear from the
\* tenant-map retry model. The commit still needs the same ID and emptiness.
SubmitDelete ==
    /\ worker.phase = "delete"
    /\ record.state = "pending"
    /\ deletion = "idle"
    /\ tenant.id = record.id
    /\ deletion' = "submitted"
    /\ UNCHANGED <<record, tenant, clear, repairs, worker>>

OriginalIsEmpty == tenant.id = OriginalId /\ tenant.metadata = "empty" /\ ~tenant.hasData

CommitDelete ==
    /\ deletion = "submitted"
    /\ OriginalIsEmpty
    /\ tenant' = [tenant EXCEPT !.id = NoTenant]
    /\ deletion' = "committed"
    /\ UNCHANGED <<record, clear, repairs, worker>>

AbortDelete ==
    /\ deletion = "submitted"
    /\ deletion' = "aborted"
    /\ UNCHANGED <<record, tenant, clear, repairs, worker>>

OtherDeleteOriginal ==
    /\ OriginalIsEmpty
    /\ tenant' = [tenant EXCEPT !.id = NoTenant]
    /\ UNCHANGED <<record, clear, deletion, repairs, worker>>

ReuseName ==
    /\ tenant.id = NoTenant
    /\ ~tenant.reused
    /\ tenant' = [id |-> ReplacementId, metadata |-> "empty", hasData |-> FALSE, reused |-> TRUE]
    /\ UNCHANGED <<record, clear, deletion, repairs, worker>>

WriteData ==
    /\ tenant.id \in TenantIds
    /\ ~tenant.hasData
    /\ tenant' = [tenant EXCEPT !.hasData = TRUE]
    /\ UNCHANGED <<record, clear, deletion, repairs, worker>>

PopulateMetadata ==
    /\ tenant.id \in TenantIds
    /\ tenant.metadata = "empty"
    /\ tenant' \in {[tenant EXCEPT !.metadata = m] : m \in {"repaired", "incompatible"}}
    /\ UNCHANGED <<record, clear, deletion, repairs, worker>>

\* A crash or abandoned response loses volatile state. Submitted commits
\* may still settle, including a repair whose success was never recorded.
StopWorker ==
    /\ worker' = WorkerAt("offline")
    /\ UNCHANGED <<record, tenant, clear, deletion, repairs>>

Crash == worker.phase # "offline" /\ StopWorker

Restart ==
    /\ worker.phase = "offline"
    /\ LET phase == CASE record.state = "pending" -> "lookup"
                       [] record.state = "absent" -> "prepare"
                       [] OTHER -> "done"
       IN worker' = WorkerAt(phase)
    /\ UNCHANGED <<record, tenant, clear, deletion, repairs>>

ReconcileName ==
    /\ worker.phase = "lookup"
    /\ IF tenant.id = record.id
       THEN /\ worker' = [worker EXCEPT !.phase = "repair"]
            /\ UNCHANGED record
       ELSE /\ worker' = [worker EXCEPT !.phase = "done"]
            /\ record' = [record EXCEPT !.state = "original-absent"]
    /\ UNCHANGED <<tenant, clear, deletion, repairs>>

\* An unavailable lookup keeps the record pending so it can be retried.
DeferReconciliation ==
    /\ worker.phase \in {"lookup", "repair"}
    /\ StopWorker

\* A successful lookup can go stale before the handle resolves. Check the
\* same handle that reads metadata and submits the recovery transaction.
SubmitRepair(i) ==
    /\ worker.phase = "repair"
    /\ repairs[i].state = "idle"
    /\ tenant.id \in TenantIds
    /\ (~VerifyRepairId \/ tenant.id = record.id)
    /\ tenant.metadata = "empty" \/ (FenceCompatibleMetadata /\ tenant.metadata \in Compatible)
    /\ repairs' = [repairs EXCEPT ![i] = [target |-> tenant.id, read |-> tenant.metadata,
                                           state |-> "submitted"]]
    /\ worker' = [worker EXCEPT !.phase = "await-repair", !.slot = i]
    /\ UNCHANGED <<record, tenant, clear, deletion>>

\* Empty metadata is initialized. Compatible metadata is preserved with a
\* write conflict against any older pending clear. The metadata read guards
\* against concurrent population and the verified handle cannot retarget.
CommitRepair(i) ==
    /\ repairs[i].state = "submitted"
    /\ repairs[i].target = tenant.id
    /\ repairs[i].read = tenant.metadata
    /\ repairs' = [repairs EXCEPT ![i].state = "committed"]
    /\ tenant' = [tenant EXCEPT !.metadata = IF @ = "empty" THEN "repaired" ELSE @]
    /\ clear' = IF clear = "submitted" THEN "aborted" ELSE clear
    /\ UNCHANGED <<record, deletion, worker>>

AbortRepair(i) ==
    /\ repairs[i].state = "submitted"
    /\ repairs' = [repairs EXCEPT ![i].state = "aborted"]
    /\ UNCHANGED <<record, tenant, clear, deletion, worker>>

\* An acknowledgement belongs to the current worker's transaction. After
\* a crash, a later repair can confirm metadata left by an earlier commit.
ObserveRepair ==
    /\ worker.phase = "await-repair"
    /\ repairs[worker.slot].state = "committed"
    /\ record' = [record EXCEPT !.state = "metadata-safe"]
    /\ worker' = [worker EXCEPT !.phase = "done", !.slot = 0]
    /\ UNCHANGED <<tenant, clear, deletion, repairs>>

AcceptCompatibleWithoutFence ==
    /\ ~FenceCompatibleMetadata
    /\ worker.phase = "repair"
    /\ tenant.id = record.id
    /\ tenant.metadata \in Compatible
    /\ record' = [record EXCEPT !.state = "metadata-safe"]
    /\ worker' = [worker EXCEPT !.phase = "done"]
    /\ UNCHANGED <<tenant, clear, deletion, repairs>>

Next ==
    \/ PersistIntent
    \/ SubmitClear \/ CommitClear \/ AbortClear \/ ObserveClear
    \/ SubmitDelete \/ CommitDelete \/ AbortDelete
    \/ OtherDeleteOriginal \/ ReuseName \/ WriteData \/ PopulateMetadata
    \/ Crash \/ Restart \/ ReconcileName \/ DeferReconciliation
    \/ \E i \in RepairSlots : SubmitRepair(i) \/ CommitRepair(i) \/ AbortRepair(i)
    \/ ObserveRepair \/ AcceptCompatibleWithoutFence

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ record \in [id : {NoTenant, OriginalId}, state : RecordStates]
    /\ tenant \in [id : MappingIds, metadata : Metadata, hasData : BOOLEAN, reused : BOOLEAN]
    /\ clear \in CommitStates
    /\ deletion \in CommitStates
    /\ repairs \in [RepairSlots -> [target : MappingIds, read : Metadata, state : CommitStates]]
    /\ worker \in [phase : WorkerPhases, slot : 0..MaxRepairs]

DestructiveWorkHasRecord ==
    (clear # "idle" \/ deletion # "idle") => (record.id = OriginalId /\ record.state # "absent")

AbsenceRecordIsAccurate == record.state = "original-absent" => tenant.id # record.id

SafeMetadataRecordIsAccurate ==
    record.state = "metadata-safe" => (tenant.id = record.id /\ tenant.metadata \in Compatible)

RepairTargetsRecordedId ==
    \A i \in RepairSlots : repairs[i].state = "committed" => repairs[i].target = record.id

RepairPreservesExistingMetadata ==
    [][(\A i \in RepairSlots :
         (repairs[i].state # "committed" /\ repairs[i]'.state = "committed" /\ tenant.metadata # "empty")
         => tenant.metadata' = tenant.metadata)]_vars

CompletedRecordIsStable ==
    [][(record.state \in {"original-absent", "metadata-safe"} => record' = record)]_vars

ReplacementSurvives == tenant.reused => tenant.id = ReplacementId
ApplicationDataSurvives == tenant.hasData => tenant.id \in TenantIds

=============================================================================
