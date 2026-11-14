--------------------------- MODULE TenantRestoration ---------------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

EXTENDS Naturals

CONSTANTS OriginalId, ReplacementId, NoTenant, BindRestore, AlwaysWrite, MaxAttempts, FocusLateDelete

ASSUME /\ OriginalId # ReplacementId
       /\ NoTenant \notin {OriginalId, ReplacementId}
       /\ BindRestore \in BOOLEAN
       /\ AlwaysWrite \in BOOLEAN
       /\ FocusLateDelete \in BOOLEAN
       /\ MaxAttempts \in Nat
       /\ MaxAttempts > 0

VARIABLES tenant, operation

vars == <<tenant, operation>>
TenantIds == {OriginalId, ReplacementId}
Names == TenantIds \cup {NoTenant}
Metadata == {"empty", "saved", "replacement"}
CommitStates == {"idle", "pending", "committed", "aborted"}
Attempts == 1..MaxAttempts
Phases == {"clear", "delete", "lookup", "restore", "await-restore", "done"}

Init ==
    /\ tenant = [id |-> OriginalId,
                 metadata |-> IF FocusLateDelete THEN "empty" ELSE "saved",
                 appData |-> FALSE]
    /\ operation = [phase |-> IF FocusLateDelete THEN "delete" ELSE "clear",
                    clear |-> IF FocusLateDelete THEN "committed" ELSE "pending",
                    clearConflicted |-> FALSE,
                    deletion |-> IF FocusLateDelete THEN "pending" ELSE "idle",
                    restore |-> [i \in Attempts |-> [target |-> NoTenant, commit |-> "idle"]],
                    used |-> 0,
                    restored |-> FALSE,
                    result |-> "pending"]

\* Start after the clear transaction has saved the original values and
\* submitted its commit. The earlier non-snapshot read protects its range.
CommitClear ==
    /\ operation.clear = "pending"
    /\ ~operation.clearConflicted
    /\ tenant.id = OriginalId
    /\ ~tenant.appData
    /\ tenant' = [tenant EXCEPT !.metadata = "empty"]
    /\ operation' = [operation EXCEPT !.clear = "committed"]

AbortClear ==
    /\ operation.clear = "pending"
    /\ operation' = [operation EXCEPT !.clear = "aborted"]
    /\ UNCHANGED tenant

ObserveClearSuccess ==
    /\ operation.phase = "clear"
    /\ operation.clear = "committed"
    /\ operation' = [operation EXCEPT !.phase = "delete", !.deletion = "pending"]
    /\ UNCHANGED tenant

\* A timeout can be reported with the clear still pending. A definite
\* failure or an unknown-but-resolved commit can take the same branch.
ObserveClearFailure ==
    /\ operation.phase = "clear"
    /\ operation' = [operation EXCEPT !.phase = "restore"]
    /\ UNCHANGED tenant

WriteApplicationData ==
    /\ tenant.id = OriginalId
    /\ ~tenant.appData
    /\ tenant' = [tenant EXCEPT !.appData = TRUE]
    /\ UNCHANGED operation

OriginalIsEmpty ==
    /\ tenant.id = OriginalId
    /\ tenant.metadata = "empty"
    /\ ~tenant.appData

CommitDelete ==
    /\ operation.deletion = "pending"
    \* The focused counterexample uses this attempt's late commit.
    /\ (~FocusLateDelete \/ operation.phase # "delete")
    /\ OriginalIsEmpty
    /\ tenant' = [tenant EXCEPT !.id = NoTenant]
    /\ operation' = [operation EXCEPT !.deletion = "committed"]

AbortDelete ==
    /\ operation.deletion = "pending"
    /\ (~FocusLateDelete \/ operation.phase # "delete")
    /\ operation' = [operation EXCEPT !.deletion = "aborted"]
    /\ UNCHANGED tenant

ObserveDeleteSuccess ==
    /\ operation.phase = "delete"
    /\ operation.deletion = "committed"
    /\ operation' = [operation EXCEPT !.phase = "done", !.result = "deleted"]
    /\ UNCHANGED tenant

\* The caller stops waiting independently of the submitted delete. A late
\* commit still requires emptiness and can race lookup and restoration.
ObserveDeleteFailure ==
    /\ operation.phase = "delete"
    /\ (~FocusLateDelete \/ operation.deletion = "pending")
    /\ operation' = [operation EXCEPT !.phase = "lookup"]
    /\ UNCHANGED tenant

LookupNameAbsent ==
    /\ operation.phase = "lookup"
    /\ tenant.id = NoTenant
    /\ operation' = [operation EXCEPT !.phase = "done", !.result = "deleted"]
    /\ UNCHANGED tenant

\* A present name and an unavailable lookup both lead to restore attempts.
LookupNeedsRestore(unavailable) ==
    /\ operation.phase = "lookup"
    /\ (unavailable \/ tenant.id \in TenantIds)
    /\ operation' = [operation EXCEPT !.phase = "restore"]
    /\ UNCHANGED tenant

OtherDeleteOriginal ==
    /\ ~FocusLateDelete
    /\ OriginalIsEmpty
    /\ tenant' = [tenant EXCEPT !.id = NoTenant]
    /\ UNCHANGED operation

ReuseName(metadata) ==
    /\ tenant.id = NoTenant
    /\ tenant' = [tenant EXCEPT !.id = ReplacementId, !.metadata = metadata]
    /\ UNCHANGED operation

StartRestore ==
    /\ operation.phase = "restore"
    /\ operation.used < MaxAttempts
    /\ LET next == operation.used + 1
           target == IF BindRestore /\ tenant.id # OriginalId THEN NoTenant ELSE tenant.id
           state == IF target = NoTenant THEN "aborted"
                    ELSE IF ~AlwaysWrite /\ tenant.metadata = "saved" THEN "skipped"
                    ELSE "pending"
       IN operation' = [operation EXCEPT !.phase = "await-restore",
                         !.used = next,
                         !.restore[next] = [target |-> target, commit |-> state]]
    /\ UNCHANGED tenant

\* Restoring the saved values is a real write even when they are already
\* present. It conflicts with an older pending clear of the same read range.
CommitRestore(i) ==
    /\ operation.restore[i].commit = "pending"
    /\ operation.restore[i].target = tenant.id
    /\ tenant.id \in TenantIds
    /\ tenant' = [tenant EXCEPT !.metadata = "saved"]
    /\ operation' = [operation EXCEPT !.restore[i].commit = "committed",
                      !.clearConflicted = @ \/ (tenant.id = OriginalId /\ operation.clear = "pending")]

AbortRestore(i) ==
    /\ operation.restore[i].commit = "pending"
    /\ operation' = [operation EXCEPT !.restore[i].commit = "aborted"]
    /\ UNCHANGED tenant

ObserveRestoreSuccess ==
    /\ operation.phase = "await-restore"
    /\ operation.restore[operation.used].commit \in {"committed", "skipped"}
    /\ operation' = [operation EXCEPT !.phase = "done", !.restored = TRUE, !.result = "failed"]
    /\ UNCHANGED tenant

\* A timed-out attempt may still settle after the caller starts another
\* attempt or exhausts the retry budget and returns.
ObserveRestoreFailure ==
    /\ operation.phase = "await-restore"
    /\ operation.restore[operation.used].commit # "skipped"
    /\ operation' = [operation EXCEPT
                      !.phase = IF operation.used = MaxAttempts THEN "done" ELSE "restore",
                      !.result = IF operation.used = MaxAttempts THEN "failed" ELSE "pending"]
    /\ UNCHANGED tenant

Quiesce == operation.phase = "done" /\ UNCHANGED vars

Next ==
    \/ CommitClear
    \/ AbortClear
    \/ ObserveClearSuccess
    \/ ObserveClearFailure
    \/ WriteApplicationData
    \/ CommitDelete
    \/ AbortDelete
    \/ ObserveDeleteSuccess
    \/ ObserveDeleteFailure
    \/ LookupNameAbsent
    \/ \E unavailable \in BOOLEAN : LookupNeedsRestore(unavailable)
    \/ OtherDeleteOriginal
    \/ \E metadata \in {"empty", "replacement"} : ReuseName(metadata)
    \/ StartRestore
    \/ \E i \in Attempts : CommitRestore(i) \/ AbortRestore(i)
    \/ ObserveRestoreSuccess
    \/ ObserveRestoreFailure
    \/ Quiesce

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ tenant \in [id : Names, metadata : Metadata, appData : BOOLEAN]
    /\ operation \in [phase : Phases,
                     clear : CommitStates,
                     clearConflicted : BOOLEAN,
                     deletion : CommitStates,
                     restore : [Attempts -> [target : Names, commit : CommitStates \cup {"skipped"}]],
                     used : 0..MaxAttempts,
                     restored : BOOLEAN,
                     result : {"pending", "failed", "deleted"}]

RestoreWritesOnlyOriginal ==
    \A i \in Attempts :
        operation.restore[i].commit = "committed" => operation.restore[i].target = OriginalId

AcknowledgedRestoreSurvives ==
    (operation.restored /\ tenant.id = OriginalId) => tenant.metadata = "saved"

ReplacementNeverReceivesSavedMetadata ==
    tenant.id = ReplacementId => tenant.metadata # "saved"

DeleteSuccessMeansOriginalAbsent ==
    operation.result = "deleted" => tenant.id # OriginalId

ApplicationDataSurvives == tenant.appData => tenant.id = OriginalId

=============================================================================
