-------------------------- MODULE TenantMapDeletion ---------------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

EXTENDS Naturals

CONSTANTS OriginalId, ReplacementId, NoTenant, MaxAttempts, CheckRetryId, AllowOtherDelete

ASSUME /\ OriginalId # ReplacementId
       /\ NoTenant \notin {OriginalId, ReplacementId}
       /\ MaxAttempts \in Nat
       /\ MaxAttempts >= 2
       /\ CheckRetryId \in BOOLEAN
       /\ AllowOtherDelete \in BOOLEAN

VARIABLES tenant, attempt, caller

vars == <<tenant, attempt, caller>>
TenantIds == {OriginalId, ReplacementId}
MappingIds == TenantIds \cup {NoTenant}
AttemptStates == {"reading", "read", "staged", "submitted", "committed", "aborted",
                   "not-empty", "no-clear"}
CallerStates == {"running", "retry", "reconcile", "succeeded", "failed"}

NewAttempt(number) == [number |-> number, readId |-> NoTenant, state |-> "reading"]

Init ==
    /\ tenant = [id |-> OriginalId, hasData |-> FALSE, reused |-> FALSE]
    /\ attempt = NewAttempt(1)
    /\ caller = "running"

\* Phase one has cleared the metadata. Application writes and a competing
\* operator can still run during any phase of the tenant-map operation.
WriteApplicationData ==
    /\ tenant.id \in TenantIds
    /\ ~tenant.hasData
    /\ tenant' = [tenant EXCEPT !.hasData = TRUE]
    /\ UNCHANGED <<attempt, caller>>

OtherDeleteOriginal ==
    /\ AllowOtherDelete
    /\ tenant.id = OriginalId
    /\ ~tenant.hasData
    /\ tenant' = [tenant EXCEPT !.id = NoTenant]
    /\ UNCHANGED <<attempt, caller>>

ReuseName ==
    /\ tenant.id = NoTenant
    /\ ~tenant.reused
    /\ tenant' = [id |-> ReplacementId, hasData |-> FALSE, reused |-> TRUE]
    /\ UNCHANGED <<attempt, caller>>

ReadTenantId ==
    /\ caller = "running"
    /\ attempt.state = "reading"
    /\ attempt' = [attempt EXCEPT !.readId = tenant.id, !.state = "read"]
    /\ UNCHANGED <<tenant, caller>>

StageClear ==
    /\ caller = "running"
    /\ attempt.state = "read"
    /\ LET checkId == attempt.number = 1 \/ CheckRetryId
           canClear == attempt.readId \in TenantIds /\ (~checkId \/ attempt.readId = OriginalId)
       IN attempt' = [attempt EXCEPT !.state = IF canClear THEN "staged" ELSE "no-clear"]
    /\ UNCHANGED <<tenant, caller>>

SubmitClear ==
    /\ caller = "running"
    /\ attempt.state = "staged"
    /\ attempt' = [attempt EXCEPT !.state = "submitted"]
    /\ UNCHANGED <<tenant, caller>>

\* The read and clear belong to the same transaction. With distinct IDs and
\* one reuse, the current ID can equal the read ID only if that entry has not
\* been removed or replaced. FoundationDB also checks emptiness at commit.
CommitClear ==
    /\ attempt.state = "submitted"
    /\ tenant.id = attempt.readId
    /\ ~tenant.hasData
    /\ tenant' = [tenant EXCEPT !.id = NoTenant]
    /\ attempt' = [attempt EXCEPT !.state = "committed"]
    /\ UNCHANGED caller

\* An abort may result from a changed mapping or a transient database error.
AbortClear ==
    /\ attempt.state = "submitted"
    /\ attempt' = [attempt EXCEPT !.state = "aborted"]
    /\ UNCHANGED <<tenant, caller>>

RejectNonempty ==
    /\ attempt.state = "submitted"
    /\ tenant.id = attempt.readId
    /\ tenant.hasData
    /\ attempt' = [attempt EXCEPT !.state = "not-empty"]
    /\ UNCHANGED <<tenant, caller>>

ObserveSuccess ==
    /\ caller = "running"
    /\ attempt.state \in {"committed", "no-clear"}
    /\ caller' = "succeeded"
    /\ UNCHANGED <<tenant, attempt>>

RetryOrReconcile == IF attempt.number < MaxAttempts THEN "retry" ELSE "reconcile"

ObserveRetryableAbort ==
    /\ caller = "running"
    /\ attempt.state = "aborted"
    /\ caller' = RetryOrReconcile
    /\ UNCHANGED <<tenant, attempt>>

\* commit_unknown_result is observed after the outcome is settled. From the
\* caller's perspective, the same response can follow a commit or an abort.
ObserveUnknownResult ==
    /\ caller = "running"
    /\ attempt.state \in {"committed", "aborted"}
    /\ caller' = RetryOrReconcile
    /\ UNCHANGED <<tenant, attempt>>

Retry ==
    /\ caller = "retry"
    /\ attempt.number < MaxAttempts
    /\ attempt' = NewAttempt(attempt.number + 1)
    /\ caller' = "running"
    /\ UNCHANGED tenant

ObserveNonempty ==
    /\ caller = "running"
    /\ attempt.state = "not-empty"
    /\ caller' = "reconcile"
    /\ UNCHANGED <<tenant, attempt>>

\* The adapter's deadline ends the retry loop even if a submitted clear is
\* still in flight. Its commit or abort remains enabled independently.
ReachDeadline ==
    /\ caller \in {"running", "retry"}
    /\ caller' = "reconcile"
    /\ UNCHANGED <<tenant, attempt>>

Reconcile ==
    /\ caller = "reconcile"
    /\ caller' = IF tenant.id = OriginalId THEN "failed" ELSE "succeeded"
    /\ UNCHANGED <<tenant, attempt>>

Quiesce == caller \in {"succeeded", "failed"} /\ UNCHANGED vars

Next ==
    \/ WriteApplicationData
    \/ OtherDeleteOriginal
    \/ ReuseName
    \/ ReadTenantId
    \/ StageClear
    \/ SubmitClear
    \/ CommitClear
    \/ AbortClear
    \/ RejectNonempty
    \/ ObserveSuccess
    \/ ObserveRetryableAbort
    \/ ObserveUnknownResult
    \/ Retry
    \/ ObserveNonempty
    \/ ReachDeadline
    \/ Reconcile
    \/ Quiesce

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ tenant \in [id : MappingIds, hasData : BOOLEAN, reused : BOOLEAN]
    /\ attempt \in [number : 1..MaxAttempts, readId : MappingIds, state : AttemptStates]
    /\ caller \in CallerStates

ReplacementSurvives == tenant.reused => tenant.id = ReplacementId

SuccessMeansOriginalMappingAbsent == caller = "succeeded" => tenant.id # OriginalId

ApplicationDataSurvives == tenant.hasData => tenant.id \in TenantIds

=============================================================================
