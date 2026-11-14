------------------------- MODULE TenantDeleteRecovery -------------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

EXTENDS Naturals

CONSTANTS OriginalId, ReplacementId, NoTenant, MaxDeleteAttempts, MaxRestoreAttempts

ASSUME /\ MaxDeleteAttempts \in Nat
       /\ MaxDeleteAttempts >= 2
       /\ MaxRestoreAttempts \in Nat
       /\ MaxRestoreAttempts > 0

VARIABLES tenant, attempt, deleteCaller, metadata, clear, recovery

Map == INSTANCE TenantMapDeletion WITH caller <- deleteCaller,
       MaxAttempts <- MaxDeleteAttempts, CheckRetryId <- TRUE, AllowOtherDelete <- TRUE

vars == <<tenant, attempt, deleteCaller, metadata, clear, recovery>>
RestoreAttempts == 1..MaxRestoreAttempts
RestoreStates == {"idle", "verified", "submitted", "committed", "aborted"}
ClearStates == {"submitted", "committed", "aborted", "conflicted"}
Returned == {"deleted", "restored", "restore-exhausted"}
Phases == {"clear", "delete", "restore", "await-restore"} \cup Returned

\* Start with a submitted clear and a nonempty snapshot. The saved values
\* are represented by one marker, including when only one kit key was present.
Init ==
    /\ Map!Init
    /\ metadata = "saved"
    /\ clear = "submitted"
    /\ recovery = [phase |-> "clear", lookup |-> "not-run", used |-> 0,
                   restore |-> [i \in RestoreAttempts |-> "idle"],
                   deleteBudgetExhausted |-> FALSE,
                   deleteAfterExhaustion |-> FALSE]

WriteApplicationData == Map!WriteApplicationData /\ UNCHANGED <<metadata, clear, recovery>>

OtherDeleteOriginal ==
    /\ metadata = "empty"
    /\ Map!OtherDeleteOriginal
    /\ UNCHANGED <<metadata, clear, recovery>>

ReuseName ==
    /\ Map!ReuseName
    /\ metadata' \in {"empty", "replacement"}
    /\ UNCHANGED <<clear, recovery>>

CommitMetadataClear ==
    /\ clear = "submitted"
    /\ tenant.id = OriginalId
    /\ ~tenant.hasData
    /\ metadata' = "empty"
    /\ clear' = "committed"
    /\ UNCHANGED <<tenant, attempt, deleteCaller, recovery>>

AbortMetadataClear ==
    /\ clear = "submitted"
    /\ clear' = "aborted"
    /\ UNCHANGED <<tenant, attempt, deleteCaller, metadata, recovery>>

ObserveMetadataClearSuccess ==
    /\ recovery.phase = "clear"
    /\ clear = "committed"
    /\ recovery' = [recovery EXCEPT !.phase = "delete"]
    /\ UNCHANGED <<tenant, attempt, deleteCaller, metadata, clear>>

\* A clear can report a failure while its commit is still pending. Phase two
\* only runs after an acknowledged clear, so this path restores directly.
ObserveMetadataClearFailure ==
    /\ recovery.phase = "clear"
    /\ recovery' = [recovery EXCEPT !.phase = "restore"]
    /\ UNCHANGED <<tenant, attempt, deleteCaller, metadata, clear>>

RunDeleteAttempt ==
    /\ recovery.phase = "delete"
    /\ \/ Map!ReadTenantId
       \/ Map!StageClear
       \/ Map!SubmitClear
       \/ /\ attempt.number < MaxDeleteAttempts
          /\ Map!ObserveRetryableAbort \/ Map!ObserveUnknownResult
       \/ Map!Retry
       \/ Map!ObserveNonempty
       \/ Map!ReachDeadline
    /\ UNCHANGED <<metadata, clear, recovery>>

ObserveDeleteBudgetExhausted ==
    /\ recovery.phase = "delete"
    /\ attempt.number = MaxDeleteAttempts
    /\ Map!ObserveRetryableAbort \/ Map!ObserveUnknownResult
    /\ recovery' = [recovery EXCEPT !.deleteBudgetExhausted = TRUE]
    /\ UNCHANGED <<metadata, clear>>

CommitTenantMapDelete ==
    /\ metadata = "empty"
    /\ Map!CommitClear
    /\ recovery' = [recovery EXCEPT
                     !.deleteAfterExhaustion = (recovery.phase = "restore-exhausted")]
    /\ UNCHANGED <<metadata, clear>>

\* A late delete still needs an empty tenant. As soon as restoration writes
\* the metadata, it can make a submitted delete fail the emptiness check.
RejectDeleteWithMetadata ==
    /\ attempt.state = "submitted"
    /\ tenant.id = attempt.readId
    /\ metadata # "empty"
    /\ attempt' = [attempt EXCEPT !.state = "not-empty"]
    /\ UNCHANGED <<tenant, deleteCaller>>

AbortTenantMapDelete ==
    /\ \/ Map!AbortClear
       \/ Map!RejectNonempty
       \/ RejectDeleteWithMetadata
    /\ UNCHANGED <<metadata, clear, recovery>>

ObserveDeleteSuccess ==
    /\ recovery.phase = "delete"
    /\ Map!ObserveSuccess
    /\ recovery' = [recovery EXCEPT !.phase = "deleted"]
    /\ UNCHANGED <<metadata, clear>>

LookupAfterDelete(lookup) ==
    /\ recovery.phase = "delete"
    /\ deleteCaller = "reconcile"
    /\ LET nextPhase == IF lookup = "available" /\ tenant.id # OriginalId
                        THEN "deleted" ELSE "restore"
       IN recovery' = [recovery EXCEPT !.phase = nextPhase, !.lookup = lookup]
    /\ UNCHANGED <<tenant, attempt, deleteCaller, metadata, clear>>

\* A verified handle carries OriginalId into its transaction. An unavailable
\* lookup does not prevent a subsequent attempt from verifying its own handle.
StartRestore ==
    /\ recovery.phase = "restore"
    /\ recovery.used < MaxRestoreAttempts
    /\ LET next == recovery.used + 1
           state == IF tenant.id = OriginalId THEN "verified" ELSE "aborted"
       IN recovery' = [recovery EXCEPT !.phase = "await-restore", !.used = next,
                        !.restore[next] = state]
    /\ UNCHANGED <<tenant, attempt, deleteCaller, metadata, clear>>

SubmitRestore ==
    /\ recovery.phase = "await-restore"
    /\ recovery.restore[recovery.used] = "verified"
    /\ recovery' = [recovery EXCEPT !.restore[recovery.used] = "submitted"]
    /\ UNCHANGED <<tenant, attempt, deleteCaller, metadata, clear>>

\* Each submitted restore is independent of the caller's deadline. Its
\* unconditional write conflicts with a still-submitted phase-one clear.
CommitRestore(i) ==
    /\ recovery.restore[i] = "submitted"
    /\ tenant.id = OriginalId
    /\ recovery' = [recovery EXCEPT !.restore[i] = "committed"]
    /\ metadata' = "saved"
    /\ clear' = IF clear = "submitted" THEN "conflicted" ELSE clear
    /\ UNCHANGED <<tenant, attempt, deleteCaller>>

AbortRestore(i) ==
    /\ recovery.restore[i] = "submitted"
    /\ recovery' = [recovery EXCEPT !.restore[i] = "aborted"]
    /\ UNCHANGED <<tenant, attempt, deleteCaller, metadata, clear>>

ObserveRestoreSuccess ==
    /\ recovery.phase = "await-restore"
    /\ recovery.restore[recovery.used] = "committed"
    /\ recovery' = [recovery EXCEPT !.phase = "restored"]
    /\ UNCHANGED <<tenant, attempt, deleteCaller, metadata, clear>>

ObserveRestoreFailure ==
    /\ recovery.phase = "await-restore"
    /\ LET exhausted == recovery.used = MaxRestoreAttempts
           state == recovery.restore[recovery.used]
       IN recovery' = [recovery EXCEPT !.phase = IF exhausted THEN "restore-exhausted" ELSE "restore",
                        !.restore[recovery.used] = IF state = "verified" THEN "aborted" ELSE state]
    /\ UNCHANGED <<tenant, attempt, deleteCaller, metadata, clear>>

Quiesce == recovery.phase \in Returned /\ UNCHANGED vars

Next ==
    \/ WriteApplicationData
    \/ OtherDeleteOriginal
    \/ ReuseName
    \/ CommitMetadataClear
    \/ AbortMetadataClear
    \/ ObserveMetadataClearSuccess
    \/ ObserveMetadataClearFailure
    \/ RunDeleteAttempt
    \/ ObserveDeleteBudgetExhausted
    \/ CommitTenantMapDelete
    \/ AbortTenantMapDelete
    \/ ObserveDeleteSuccess
    \/ \E lookup \in {"available", "unavailable"} : LookupAfterDelete(lookup)
    \/ StartRestore
    \/ SubmitRestore
    \/ \E i \in RestoreAttempts : CommitRestore(i) \/ AbortRestore(i)
    \/ ObserveRestoreSuccess
    \/ ObserveRestoreFailure
    \/ Quiesce

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ Map!TypeOK
    /\ metadata \in {"empty", "saved", "replacement"}
    /\ clear \in ClearStates
    /\ recovery \in [phase : Phases, lookup : {"not-run", "available", "unavailable"},
                    used : 0..MaxRestoreAttempts, restore : [RestoreAttempts -> RestoreStates],
                    deleteBudgetExhausted : BOOLEAN,
                    deleteAfterExhaustion : BOOLEAN]

ReplacementSurvives == Map!ReplacementSurvives
ApplicationDataSurvives == Map!ApplicationDataSurvives
ReplacementNeverGetsSavedMetadata == tenant.id = ReplacementId => metadata # "saved"
DeleteSuccessMeansOriginalAbsent == recovery.phase = "deleted" => tenant.id # OriginalId

CommittedRestoreProtectsOriginal ==
    (\E i \in RestoreAttempts : recovery.restore[i] = "committed")
    => (tenant.id = OriginalId /\ metadata = "saved")

\* These stronger guarantees are checked separately as scenario witnesses.
ExhaustionPreservesMetadata ==
    (/\ recovery.phase = "restore-exhausted"
     /\ recovery.lookup = "unavailable"
     /\ recovery.deleteBudgetExhausted
     /\ attempt.state = "aborted"
     /\ \A i \in RestoreAttempts : recovery.restore[i] = "aborted"
     /\ tenant.id = OriginalId)
    => metadata = "saved"

NoDeleteCommitAfterExhaustion ==
    recovery.lookup = "unavailable" => ~recovery.deleteAfterExhaustion

=============================================================================
