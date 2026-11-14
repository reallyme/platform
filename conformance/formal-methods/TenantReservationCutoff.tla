------------------------ MODULE TenantReservationCutoff ------------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

EXTENDS Naturals

CONSTANTS CheckCutoffAtCommit, RequireCutoffForReclaim
ASSUME /\ CheckCutoffAtCommit \in BOOLEAN
       /\ RequireCutoffForReclaim \in BOOLEAN

Operations == {"OpA", "OpB"}
NoOperation == "none"
NoTenant == 0
OriginalId == 1
ReplacementId == 2
TenantIds == {OriginalId, ReplacementId}
Generations == 0..2
AdvanceTargets == Generations \ {0}
Live == {"deleting", "recovering"}
Terminal == {"complete", "tombstone"}
Outcomes == {"metadata-safe", "original-absent"}
Replies == {"acknowledged", "unknown"}
CommittedAdvances == Replies \cup {"retried", "retried-later"}
Missing == [phase |-> "missing", id |-> NoTenant, generation |-> 0, outcome |-> "none"]

VARIABLES tenant, cutoff, intents, active, records, advances, history

vars == <<tenant, cutoff, intents, active, records, advances, history>>

Init ==
    /\ tenant = [id |-> OriginalId, reused |-> FALSE]
    /\ cutoff = 0
    /\ intents = [op \in Operations |-> [id |-> NoTenant, generation |-> 0,
                                         state |-> "empty", prepared |-> "none"]]
    /\ active = NoOperation
    /\ records = [op \in Operations |-> Missing]
    /\ advances = [g \in AdvanceTargets |-> "none"]
    /\ history = [op \in Operations |-> [completed |-> "none", reclaimReply |-> "none"]]

\* Capture the tenant ID and the admission generation in one cluster read.
\* Local persistence can finish after the cutoff moves. The file keeps the
\* generation it captured. Pre-generation files have generation zero.
CaptureIntent(op) ==
    /\ intents[op].state = "empty"
    /\ tenant.id \in TenantIds
    /\ intents' = [intents EXCEPT ![op].id = tenant.id, ![op].generation = cutoff,
                                  ![op].state = "captured"]
    /\ UNCHANGED <<tenant, cutoff, active, records, advances, history>>

PersistIntent(op) ==
    /\ intents[op].state = "captured"
    /\ intents' = [intents EXCEPT ![op].state = "pending"]
    /\ UNCHANGED <<tenant, cutoff, active, records, advances, history>>

PrepareClaim(op, phase) ==
    /\ intents[op].state = "pending"
    /\ intents[op].prepared = "none"
    /\ intents[op].generation = cutoff
    /\ intents' = [intents EXCEPT ![op].prepared = phase]
    /\ UNCHANGED <<tenant, cutoff, active, records, advances, history>>

\* This action represents the claim transaction and each retry. The
\* non-snapshot reads of the cutoff, active key, and operation key must
\* serialize with cutoff advancement and reclamation.
CommitClaim(op) ==
    /\ intents[op].state = "pending"
    /\ intents[op].prepared \in Live
    /\ (~CheckCutoffAtCommit \/ intents[op].generation = cutoff)
    /\ active = NoOperation
    /\ records[op] = Missing
    /\ (intents[op].prepared = "deleting" => tenant.id = intents[op].id)
    /\ records' = [records EXCEPT ![op] =
        [phase |-> intents[op].prepared, id |-> intents[op].id,
         generation |-> intents[op].generation, outcome |-> "none"]]
    /\ active' = op
    /\ intents' = [intents EXCEPT ![op].prepared = "none"]
    /\ UNCHANGED <<tenant, cutoff, advances, history>>

\* Older models check the lease, epoch, and separate guarded mutations.
\* Here takeover preserves the admission generation of the active grant.
TakeOver(op) ==
    /\ active = op
    /\ records[op].phase = "deleting"
    /\ records' = [records EXCEPT ![op].phase = "recovering"]
    /\ UNCHANGED <<tenant, cutoff, intents, active, advances, history>>

DeleteTenant(op) ==
    /\ active = op
    /\ records[op].phase = "deleting"
    /\ tenant.id = records[op].id
    /\ tenant' = [tenant EXCEPT !.id = NoTenant]
    /\ UNCHANGED <<cutoff, intents, active, records, advances, history>>

CommitCompletion(op) ==
    /\ active = op
    /\ (tenant.id = records[op].id => records[op].phase = "recovering")
    /\ LET outcome == IF tenant.id = records[op].id THEN "metadata-safe" ELSE "original-absent"
       IN /\ records' = [records EXCEPT ![op].phase = "complete", ![op].outcome = outcome]
          /\ history' = [history EXCEPT ![op].completed = outcome]
    /\ active' = NoOperation
    \* The cluster can complete while the original local file stays pending.
    /\ UNCHANGED <<tenant, cutoff, intents, advances>>

ReuseName ==
    /\ tenant.id = NoTenant
    /\ ~tenant.reused
    /\ tenant' = [id |-> ReplacementId, reused |-> TRUE]
    /\ UNCHANGED <<cutoff, intents, active, records, advances, history>>

PrepareAdvance(target) ==
    /\ target = cutoff + 1
    /\ advances[target] = "none"
    /\ advances' = [advances EXCEPT ![target] = "prepared"]
    /\ UNCHANGED <<tenant, cutoff, intents, active, records, history>>

\* Bind the request to this target. Draining the active key and comparing
\* the prior cutoff in the transaction orders the advance with new claims.
CommitAdvance(target, reply) ==
    /\ advances[target] = "prepared"
    /\ target = cutoff + 1
    /\ active = NoOperation
    /\ cutoff' = target
    /\ advances' = [advances EXCEPT ![target] = reply]
    /\ UNCHANGED <<tenant, intents, active, records, history>>

RetryAdvance(target) ==
    /\ advances[target] = "unknown"
    /\ cutoff >= target
    /\ advances' = [advances EXCEPT ![target] =
        IF cutoff = target THEN "retried" ELSE "retried-later"]
    /\ UNCHANGED <<tenant, cutoff, intents, active, records, history>>

Compact(op) ==
    /\ records[op].phase = "complete"
    /\ records' = [records EXCEPT ![op].phase = "tombstone"]
    /\ UNCHANGED <<tenant, cutoff, intents, active, advances, history>>

\* A collector checks the stored generation against the durable cutoff in
\* the same transaction that removes the tombstone.
Reclaim(op, reply) ==
    /\ records[op].phase = "tombstone"
    /\ (~RequireCutoffForReclaim \/ records[op].generation < cutoff)
    /\ records' = [records EXCEPT ![op] = Missing]
    /\ history' = [history EXCEPT ![op].reclaimReply = reply]
    /\ UNCHANGED <<tenant, cutoff, intents, active, advances>>

\* The retry remembers the original request generation. Absence below the
\* cutoff is already retired. A failed commit before a change can stutter.
RetryReclaim(op) ==
    /\ history[op].reclaimReply = "unknown"
    /\ records[op] = Missing
    /\ intents[op].generation < cutoff
    /\ history' = [history EXCEPT ![op].reclaimReply = "retried"]
    /\ UNCHANGED <<tenant, cutoff, intents, active, records, advances>>

ReadOutcome(op) ==
    /\ intents[op].state = "pending"
    /\ records[op].phase \in Terminal
    /\ records[op].generation = intents[op].generation
    /\ records[op].id = intents[op].id
    /\ intents' = [intents EXCEPT ![op].state = records[op].outcome, ![op].prepared = "none"]
    /\ UNCHANGED <<tenant, cutoff, active, records, advances, history>>

RejectExpired(op) ==
    /\ intents[op].state = "pending"
    /\ intents[op].generation < cutoff
    /\ records[op] = Missing
    /\ intents' = [intents EXCEPT ![op].state = "expired", ![op].prepared = "none"]
    /\ UNCHANGED <<tenant, cutoff, active, records, advances, history>>

Next ==
    \/ \E op \in Operations :
          \/ CaptureIntent(op) \/ PersistIntent(op)
          \/ \E phase \in Live : PrepareClaim(op, phase)
          \/ CommitClaim(op) \/ TakeOver(op) \/ DeleteTenant(op) \/ CommitCompletion(op)
          \/ Compact(op)
          \/ \E reply \in Replies : Reclaim(op, reply)
          \/ RetryReclaim(op) \/ ReadOutcome(op) \/ RejectExpired(op)
    \/ \E target \in AdvanceTargets :
          \/ PrepareAdvance(target) \/ RetryAdvance(target)
          \/ \E reply \in Replies : CommitAdvance(target, reply)
    \/ ReuseName

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ tenant \in [id : TenantIds \cup {NoTenant}, reused : BOOLEAN]
    /\ cutoff \in Generations
    /\ active \in Operations \cup {NoOperation}
    /\ intents \in [Operations -> [id : TenantIds \cup {NoTenant}, generation : Generations,
                                   state : Outcomes \cup {"empty", "captured", "pending", "expired"},
                                   prepared : Live \cup {"none"}]]
    /\ records \in [Operations -> [phase : Live \cup Terminal \cup {"missing"},
                                   id : TenantIds \cup {NoTenant}, generation : Generations,
                                   outcome : Outcomes \cup {"none"}]]
    /\ advances \in [AdvanceTargets -> CommittedAdvances \cup {"none", "prepared"}]
    /\ history \in [Operations -> [completed : Outcomes \cup {"none"},
                                   reclaimReply : Replies \cup {"none", "retried"}]]

CutoffHasCommittedAdvance ==
    \A g \in AdvanceTargets : (g <= cutoff) <=> (advances[g] \in CommittedAdvances)

CurrentGrantMatches ==
    \A op \in Operations :
        /\ ((active = op) <=> (records[op].phase \in Live))
        /\ (active = op =>
             /\ records[op].generation = intents[op].generation
             /\ records[op].generation = cutoff
             /\ records[op].id = intents[op].id)

NoCompletedIntentRegainsOwnership ==
    \A op \in Operations : active = op => history[op].completed = "none"

ReclamationLeavesIntentsExpired ==
    \A op \in Operations :
        history[op].reclaimReply # "none" =>
            /\ records[op] = Missing
            /\ intents[op].generation < cutoff

OutcomesStayAccurate ==
    \A op \in Operations :
        /\ (intents[op].state \in Outcomes => intents[op].state = history[op].completed)
        /\ (history[op].completed \in Outcomes /\ history[op].reclaimReply = "none" =>
             /\ records[op].phase \in Terminal
             /\ records[op].outcome = history[op].completed
             /\ records[op].id = intents[op].id
             /\ records[op].generation = intents[op].generation)

CutoffNeverDecreases == [][cutoff' >= cutoff]_vars
IntentKeepsCapturedGeneration ==
    [][(\A op \in Operations :
          intents[op].state # "empty" => intents'[op].generation = intents[op].generation)]_vars

\* Require a prepared stale resume to race a committed but unacknowledged
\* cutoff, reclamation, and a complete lifecycle of the replacement.
NoStaleClaimAfterNameReuse ==
    ~(\E old \in Operations :
        /\ active = old
        /\ records[old].phase = "recovering"
        /\ intents[old].id = OriginalId
        /\ intents[old].generation < cutoff
        /\ advances[1] = "unknown"
        /\ history[old].reclaimReply = "unknown"
        /\ tenant.id = ReplacementId
        /\ \E newer \in Operations \ {old} :
               intents[newer].id = ReplacementId /\ history[newer].completed \in Outcomes)

\* Reclaiming too early destroys the only fence for a still-eligible file.
NoSameGenerationClaimAfterReclaim ==
    ~(\E op \in Operations :
        /\ active = op
        /\ intents[op].generation = cutoff
        /\ history[op].reclaimReply = "unknown"
        /\ intents[op].id = OriginalId /\ tenant.id = OriginalId)

\* Force progress through a lost advance reply, a second advance, name
\* reuse, another completion, and an idempotent reclamation retry.
NoProgressAfterCutoff ==
    ~(/\ advances[1] = "retried-later"
      /\ tenant.id = ReplacementId
      /\ \E old \in Operations :
           /\ intents[old].id = OriginalId
           /\ intents[old].state = "expired"
           /\ history[old].reclaimReply = "retried"
           /\ \E newer \in Operations \ {old} :
                  /\ intents[newer].id = ReplacementId
                  /\ intents[newer].generation = cutoff
                  /\ intents[newer].state = "metadata-safe")

=============================================================================
