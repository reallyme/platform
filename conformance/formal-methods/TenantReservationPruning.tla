----------------------- MODULE TenantReservationPruning -----------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

EXTENDS Naturals

CONSTANT KeepTombstone
ASSUME KeepTombstone \in BOOLEAN

Operations == {"OpA", "OpB"}
NoOperation == "none"
NoTenant == 0
OriginalId == 1
ReplacementId == 2
TenantIds == {OriginalId, ReplacementId}
Live == {"deleting", "recovering"}
Terminal == {"complete", "retired"}
Outcomes == {"metadata-safe", "original-absent"}
Pending == "pending"

VARIABLES tenant, intents, active, records, history

vars == <<tenant, intents, active, records, history>>
Missing == [phase |-> "missing", id |-> NoTenant, epoch |-> 0, outcome |-> Pending]

Init ==
    /\ tenant = [id |-> OriginalId, metadata |-> TRUE, reused |-> FALSE]
    /\ intents = [op \in Operations |-> [id |-> NoTenant, outcome |-> Pending]]
    /\ active = NoOperation
    /\ records = [op \in Operations |-> Missing]
    \* History is only used to check properties and witness paths. It
    \* cannot authorize a write or tell a retry whether a row committed.
    /\ history = [op \in Operations |-> [outcome |-> Pending, pruneReply |-> "none",
                                         pruneOwner |-> NoOperation,
                                         retiredReplayOwner |-> NoOperation]]

\* The operation ID and tenant ID in a durable local file survive restart.
\* Each ID represents a distinct operation, even if both record the same tenant.
PersistIntent(op) ==
    /\ intents[op].id = NoTenant
    /\ tenant.id \in TenantIds
    /\ intents' = [intents EXCEPT ![op].id = tenant.id]
    /\ UNCHANGED <<tenant, active, records, history>>

\* A missing row can mean the file was written before the first acquisition
\* committed. Resume in this case can claim recovery even if the ID is absent.
\* Acquisition for delete also requires the recorded ID to own the name.
Claim(op, phase) ==
    /\ intents[op].id \in TenantIds
    /\ intents[op].outcome = Pending
    /\ active = NoOperation
    /\ records[op].phase = "missing"
    /\ (phase = "deleting" => tenant.id = intents[op].id)
    /\ active' = op
    /\ records' = [records EXCEPT ![op] = [phase |-> phase, id |-> intents[op].id,
                                            epoch |-> 1, outcome |-> Pending]]
    /\ UNCHANGED <<tenant, intents, history>>

\* The lease and repeated takeovers are covered by TenantReservation. A single
\* transfer here is enough to allow recovery after interrupted deletion.
TakeOver(op) ==
    /\ active = op
    /\ records[op].phase = "deleting"
    /\ records' = [records EXCEPT ![op].phase = "recovering", ![op].epoch = 2]
    /\ UNCHANGED <<tenant, intents, active, history>>

HasGrant(op, epoch) ==
    /\ active = op
    /\ records[op].phase \in Live
    /\ records[op].epoch = epoch

\* Metadata clear and tenant-map delete are abstracted into their net effect.
\* Their independent commits and conflicts with takeover are modeled separately.
DeleteTenant(op, epoch) ==
    /\ HasGrant(op, epoch)
    /\ records[op].phase = "deleting"
    /\ tenant.id = records[op].id
    /\ tenant' = [tenant EXCEPT !.id = NoTenant, !.metadata = FALSE]
    /\ UNCHANGED <<intents, active, records, history>>

Complete(op, epoch) ==
    /\ HasGrant(op, epoch)
    /\ LET stillOriginal == tenant.id = records[op].id
           outcome == IF stillOriginal THEN "metadata-safe" ELSE "original-absent"
       IN /\ (stillOriginal => records[op].phase = "recovering")
          /\ tenant' = IF stillOriginal THEN [tenant EXCEPT !.metadata = TRUE] ELSE tenant
          /\ records' = [records EXCEPT ![op].phase = "complete", ![op].outcome = outcome]
          /\ history' = [history EXCEPT ![op].outcome = outcome]
    /\ active' = NoOperation
    \* The database may commit while the caller's file still says pending.
    /\ UNCHANGED intents

ReuseName ==
    /\ tenant.id = NoTenant
    /\ ~tenant.reused
    /\ tenant' = [id |-> ReplacementId, metadata |-> FALSE, reused |-> TRUE]
    /\ UNCHANGED <<intents, active, records, history>>

\* Read the exact operation row in the pruning transaction. Replacing it
\* atomically leaves the original ID and outcome available to resumed callers
\* and discards the grant epoch. The active key can already belong to another
\* operation and pruning has no reason to write it.
Prune(op, reply) ==
    /\ records[op].phase = "complete"
    /\ records' = [records EXCEPT ![op] =
        IF KeepTombstone THEN [@ EXCEPT !.phase = "retired", !.epoch = 0] ELSE Missing]
    /\ history' = [history EXCEPT ![op].pruneReply = reply, ![op].pruneOwner = active]
    /\ UNCHANGED <<tenant, intents, active>>

\* A retry after a lost prune reply reads the tombstone and succeeds
\* idempotently. A commit that did not apply is represented by stuttering.
ObservePruneRetry(op) ==
    /\ records[op].phase = "retired"
    /\ history[op].pruneReply = "unknown"
    /\ history' = [history EXCEPT ![op].pruneReply = "retried"]
    /\ UNCHANGED <<tenant, intents, active, records>>

ReplayOutcome(op) ==
    /\ intents[op].outcome = Pending
    /\ records[op].phase \in Terminal
    /\ records[op].id = intents[op].id
    /\ intents' = [intents EXCEPT ![op].outcome = records[op].outcome]
    /\ history' = [history EXCEPT ![op].retiredReplayOwner =
        IF /\ records[op].phase = "retired"
           /\ history[op].pruneReply = "retried"
           /\ history[op].pruneOwner = active
        THEN active ELSE NoOperation]
    /\ UNCHANGED <<tenant, active, records>>

Next ==
    \/ \E op \in Operations :
          \/ PersistIntent(op)
          \/ \E phase \in Live : Claim(op, phase)
          \/ TakeOver(op)
          \/ \E epoch \in 1..2 : DeleteTenant(op, epoch) \/ Complete(op, epoch)
          \/ \E reply \in {"acknowledged", "unknown"} : Prune(op, reply)
          \/ ObservePruneRetry(op)
          \/ ReplayOutcome(op)
    \/ ReuseName

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ tenant \in [id : TenantIds \cup {NoTenant}, metadata : BOOLEAN, reused : BOOLEAN]
    /\ intents \in [Operations -> [id : TenantIds \cup {NoTenant}, outcome : Outcomes \cup {Pending}]]
    /\ active \in Operations \cup {NoOperation}
    /\ records \in [Operations -> [phase : Live \cup Terminal \cup {"missing"}, id : TenantIds \cup {NoTenant},
                                   epoch : 0..2, outcome : Outcomes \cup {Pending}]]
    /\ history \in [Operations -> [outcome : Outcomes \cup {Pending}, pruneOwner : Operations \cup {NoOperation},
                                   retiredReplayOwner : Operations \cup {NoOperation},
                                   pruneReply : {"none", "acknowledged", "unknown", "retried"}]]

ActivePointerMatches ==
    \A op \in Operations : (active = op) <=> (records[op].phase \in Live)

NoCompletedIntentRegainsOwnership ==
    \A op \in Operations : active = op => history[op].outcome = Pending

CompletedOutcomeIsReplayable ==
    \A op \in Operations :
        history[op].outcome \in Outcomes =>
            /\ records[op].phase \in Terminal
            /\ records[op].id = intents[op].id
            /\ records[op].outcome = history[op].outcome
            /\ intents[op].outcome \in {Pending, history[op].outcome}

\* Require a stale resume after an unobserved completion and an
\* ambiguous prune while the original ID still owns the name.
NoReacquisitionAfterAmbiguousCommits ==
    ~(\E op \in Operations :
        /\ active = op
        /\ records[op].phase = "recovering"
        /\ tenant.id = OriginalId
        /\ intents[op].id = OriginalId
        /\ history[op].outcome \in Outcomes
        /\ intents[op].outcome = Pending
        /\ history[op].pruneReply = "unknown")

\* A tenant-ID check on the destructive path alone cannot protect the active
\* key. A stale recovery can claim it after a newer lifecycle has completed.
NoReacquisitionAfterNameReuse ==
    ~(\E op \in Operations :
        /\ active = op
        /\ records[op].phase = "recovering"
        /\ tenant.id = ReplacementId
        /\ intents[op].id = OriginalId
        /\ history[op].outcome \in Outcomes
        /\ \E newer \in Operations \ {op} :
               intents[newer].id = ReplacementId /\ history[newer].outcome \in Outcomes)

\* Exercise pruning, its retry, and replay from the resulting tombstone
\* while a newer operation owns the reused name.
NoRetiredReplayDuringNewReservation ==
    ~(\E old \in Operations :
        /\ active \in Operations \ {old}
        /\ tenant.id = ReplacementId
        /\ records[active].id = ReplacementId
        /\ intents[old].id = OriginalId
        /\ history[old].retiredReplayOwner = active
        /\ intents[old].outcome = "original-absent")

=============================================================================
