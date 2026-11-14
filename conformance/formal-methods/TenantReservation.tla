--------------------------- MODULE TenantReservation ---------------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

EXTENDS Naturals, FiniteSets

CONSTANTS Hosts, NoHost, OriginalId, ReplacementId, NoTenant, MaxEpoch, CheckAtCommit

ASSUME /\ Cardinality(Hosts) >= 2
       /\ NoHost \notin Hosts
       /\ Cardinality({OriginalId, ReplacementId, NoTenant}) = 3
       /\ MaxEpoch \in Nat
       /\ MaxEpoch >= 2
       /\ CheckAtCommit \in BOOLEAN

VARIABLES intents, reservation, worker, tenant, transactions

vars == <<intents, reservation, worker, tenant, transactions>>
Active == {"deleting", "recovering"}
Completed == {"original-absent", "metadata-safe"}
CommitStates == {"idle", "submitted", "committed", "aborted"}
WorkerStates == {"idle", "claiming", "unconfirmed", "active", "offline"}
NoWorker(state) == [state |-> state, epoch |-> 0]

Init ==
    /\ intents = {}
    /\ reservation = [state |-> "free", epoch |-> 0, owner |-> NoHost,
                      firstOwner |-> NoHost, id |-> NoTenant, expired |-> FALSE,
                      repairedId |-> NoTenant, competingClaim |-> FALSE]
    /\ worker = [h \in Hosts |-> NoWorker("idle")]
    /\ tenant = [id |-> OriginalId, metadata |-> TRUE, hasData |-> FALSE, reused |-> FALSE]
    /\ transactions = [clear |-> "idle", delete |-> "idle", staleCommit |-> FALSE]

\* Operators can both record local intent. Only the cluster reservation
\* grants permission to begin a delete for this name and tenant ID.
PersistIntent(h) ==
    /\ worker[h].state = "idle"
    /\ h \notin intents
    /\ tenant.id = OriginalId
    /\ intents' = intents \cup {h}
    /\ UNCHANGED <<reservation, worker, tenant, transactions>>

RequestReservation(h) ==
    /\ h \in intents
    /\ worker[h].state = "idle"
    /\ reservation.state = "free"
    /\ tenant.id = OriginalId
    /\ worker' = [worker EXCEPT ![h].state = "claiming"]
    /\ UNCHANGED <<intents, reservation, tenant, transactions>>

\* Both requests can read a free key. The conditional commit permits only
\* one to write the initial reservation and leave the other without a grant.
Acquire(h) ==
    /\ worker[h].state = "claiming"
    /\ reservation.state = "free"
    /\ tenant.id = OriginalId
    /\ reservation' = [reservation EXCEPT !.state = "deleting", !.epoch = 1,
                       !.owner = h, !.firstOwner = h, !.id = tenant.id,
                       !.competingClaim = (\E other \in Hosts \ {h} : worker[other].state = "claiming")]
    /\ worker' = [worker EXCEPT ![h] = [state |-> "unconfirmed", epoch |-> 1]]
    /\ UNCHANGED <<intents, tenant, transactions>>

LoseReservation(h) ==
    /\ worker[h].state = "claiming"
    /\ reservation.state # "free"
    /\ worker' = [worker EXCEPT ![h] = NoWorker("idle")]
    /\ UNCHANGED <<intents, reservation, tenant, transactions>>

HasCurrentGrant(h) == reservation.owner = h /\ reservation.epoch = worker[h].epoch

ObserveGrant(h) ==
    /\ worker[h].state = "unconfirmed"
    /\ HasCurrentGrant(h)
    /\ worker' = [worker EXCEPT ![h].state = "active"]
    /\ UNCHANGED <<intents, reservation, tenant, transactions>>

Crash(h) ==
    /\ worker[h].state # "offline"
    /\ worker' = [worker EXCEPT ![h] = NoWorker("offline")]
    /\ UNCHANGED <<intents, reservation, tenant, transactions>>

Restart(h) ==
    /\ worker[h].state = "offline"
    /\ worker' = [worker EXCEPT ![h] = NoWorker("idle")]
    /\ UNCHANGED <<intents, reservation, tenant, transactions>>

\* Expiry abstracts the passage of time without a database write. It permits
\* transfer even if the old process is live. The new epoch revokes authority.
Expire ==
    /\ reservation.state \in Active
    /\ ~reservation.expired
    /\ reservation' = [reservation EXCEPT !.expired = TRUE]
    /\ UNCHANGED <<intents, worker, tenant, transactions>>

TakeOver(h) ==
    /\ reservation.state \in Active
    /\ reservation.expired
    /\ reservation.epoch < MaxEpoch
    /\ worker[h].state # "offline"
    /\ LET epoch == reservation.epoch + 1
       IN /\ reservation' = [reservation EXCEPT !.state = "recovering", !.epoch = epoch,
                             !.owner = h, !.expired = FALSE]
          /\ worker' = [worker EXCEPT ![h] = [state |-> "unconfirmed", epoch |-> epoch]]
    /\ UNCHANGED <<intents, tenant, transactions>>

CanStartDelete(h) ==
    /\ reservation.state = "deleting"
    /\ HasCurrentGrant(h)
    /\ worker[h].state = "active"
    /\ tenant.id = reservation.id

SubmitClear(h) ==
    /\ CanStartDelete(h)
    /\ transactions.clear = "idle"
    /\ ~tenant.hasData
    /\ transactions' = [transactions EXCEPT !.clear = "submitted"]
    /\ UNCHANGED <<intents, reservation, worker, tenant>>

SubmitDelete(h) ==
    /\ CanStartDelete(h)
    /\ transactions.clear = "committed"
    /\ transactions.delete = "idle"
    /\ transactions' = [transactions EXCEPT !.delete = "submitted"]
    /\ UNCHANGED <<intents, reservation, worker, tenant>>

\* Only the first epoch starts destructive work. Its transactions read the
\* reservation in the same transaction as their mutations. A transfer or
\* completion conflicts with those reads, even after a client crash.
FirstGrantCurrent == reservation.epoch = 1 /\ reservation.state = "deleting"

CommitDestruction(kind) ==
    /\ transactions[kind] = "submitted"
    /\ (~CheckAtCommit \/ FirstGrantCurrent)
    /\ tenant.id = reservation.id
    /\ ~tenant.hasData
    /\ IF kind = "clear"
       THEN tenant' = [tenant EXCEPT !.metadata = FALSE]
       ELSE /\ ~tenant.metadata
            /\ tenant' = [tenant EXCEPT !.id = NoTenant]
    /\ transactions' = [transactions EXCEPT ![kind] = "committed",
                        !.staleCommit = @ \/ ~FirstGrantCurrent]
    /\ UNCHANGED <<intents, reservation, worker>>

AbortDestruction(kind) ==
    /\ transactions[kind] = "submitted"
    /\ transactions' = [transactions EXCEPT ![kind] = "aborted"]
    /\ UNCHANGED <<intents, reservation, worker, tenant>>

FinishDelete(h) ==
    /\ reservation.state = "deleting"
    /\ HasCurrentGrant(h)
    /\ worker[h].state = "active"
    /\ transactions.delete = "committed"
    /\ reservation' = [reservation EXCEPT !.state = "original-absent"]
    /\ UNCHANGED <<intents, worker, tenant, transactions>>

\* Recovery uses the cluster's recorded ID, including on a different host.
\* The reservation check, metadata write, and completion serialize together.
FinishRecovery(h) ==
    /\ reservation.state = "recovering"
    /\ HasCurrentGrant(h)
    /\ worker[h].state = "active"
    /\ IF tenant.id = reservation.id
       THEN /\ tenant' = [tenant EXCEPT !.metadata = TRUE]
            /\ reservation' = [reservation EXCEPT !.state = "metadata-safe", !.repairedId = tenant.id]
       ELSE /\ UNCHANGED tenant
            /\ reservation' = [reservation EXCEPT !.state = "original-absent"]
    /\ UNCHANGED <<intents, worker, transactions>>

ReuseName ==
    /\ tenant.id = NoTenant
    /\ ~tenant.reused
    /\ tenant' = [id |-> ReplacementId, metadata |-> FALSE, hasData |-> FALSE, reused |-> TRUE]
    /\ UNCHANGED <<intents, reservation, worker, transactions>>

WriteData ==
    /\ tenant.id # NoTenant
    /\ ~tenant.hasData
    /\ tenant' = [tenant EXCEPT !.hasData = TRUE]
    /\ UNCHANGED <<intents, reservation, worker, transactions>>

Next ==
    \/ \E h \in Hosts :
          \/ PersistIntent(h) \/ RequestReservation(h) \/ Acquire(h) \/ LoseReservation(h) \/ ObserveGrant(h)
          \/ Crash(h) \/ Restart(h) \/ TakeOver(h)
          \/ SubmitClear(h) \/ SubmitDelete(h) \/ FinishDelete(h) \/ FinishRecovery(h)
    \/ \E kind \in {"clear", "delete"} : CommitDestruction(kind) \/ AbortDestruction(kind)
    \/ Expire \/ ReuseName \/ WriteData

Spec == Init /\ [][Next]_vars

\* If a recovery owner stays online and retains the reservation, eventual
\* access to the cluster lets it observe the grant and reconcile the intent.
FairSpec ==
    /\ Spec
    /\ \A h \in Hosts : WF_vars(ObserveGrant(h)) /\ WF_vars(FinishRecovery(h))

TypeOK ==
    /\ intents \subseteq Hosts
    /\ reservation \in [state : {"free"} \cup Active \cup Completed,
                        epoch : 0..MaxEpoch, owner : Hosts \cup {NoHost}, firstOwner : Hosts \cup {NoHost},
                        id : {NoTenant, OriginalId}, expired : BOOLEAN, repairedId : {NoTenant, OriginalId},
                        competingClaim : BOOLEAN]
    /\ worker \in [Hosts -> [state : WorkerStates, epoch : 0..MaxEpoch]]
    /\ tenant \in [id : {OriginalId, ReplacementId, NoTenant}, metadata : BOOLEAN,
                   hasData : BOOLEAN, reused : BOOLEAN]
    /\ transactions \in [clear : CommitStates, delete : CommitStates, staleCommit : BOOLEAN]

SingleCurrentOwner ==
    Cardinality({h \in Hosts : worker[h].state = "active" /\ worker[h].epoch = reservation.epoch}) <= 1

DestructionRequiresIntent ==
    (transactions.clear # "idle" \/ transactions.delete # "idle")
    => (reservation.firstOwner \in intents /\ reservation.id = OriginalId)

StaleTransactionsCannotCommit == ~transactions.staleCommit
RepairTargetsOriginal == reservation.repairedId \in {NoTenant, reservation.id}
ReplacementSurvives == tenant.reused => tenant.id = ReplacementId /\ ~tenant.metadata
ApplicationDataSurvives == tenant.hasData => tenant.id # NoTenant

CompletedOutcomeIsAccurate ==
    /\ (reservation.state = "original-absent" => tenant.id # reservation.id)
    /\ (reservation.state = "metadata-safe" => tenant.id = reservation.id /\ tenant.metadata)

ReservationNeverReverts ==
    [][(/\ reservation'.epoch >= reservation.epoch
        /\ (reservation.state \in Completed => reservation' = reservation))]_vars

RecoveryCompletes ==
    \A h \in Hosts :
        (<>(reservation.state = "recovering" /\ reservation.owner = h)
         /\ <>[](HasCurrentGrant(h) /\ worker[h].state \in {"unconfirmed", "active"}))
        => <>(reservation.state \in Completed)

\* Require an overlapping claim and recovery by the other host while the
\* crashed winner's delete is still in flight.
NoCrossHostCrashRecovery ==
    ~(/\ reservation.state = "metadata-safe"
       /\ reservation.owner # reservation.firstOwner
       /\ reservation.competingClaim
       /\ worker[reservation.firstOwner].state = "offline"
       /\ transactions.delete = "submitted")

=============================================================================
