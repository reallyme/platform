------------------------- MODULE FdbTransactionDeadline -------------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

EXTENDS Naturals

CONSTANTS RecheckDeadline, RejectAmbiguousWrite

Deadline == 2
AttemptLimit == 2
Modes == {"read", "write"}
Phases == {"ready", "callback", "backoff", "commit", "returned"}

VARIABLES mode, now, phase, attempts, commit, effects, reply, evidence

vars == <<mode, now, phase, attempts, commit, effects, reply, evidence>>

Init ==
    /\ mode \in Modes
    /\ now = 0
    /\ phase = "ready"
    /\ attempts = 0
    /\ commit = "none"
    /\ effects = 0
    /\ reply = "none"
    /\ evidence = [lateAttempt |-> FALSE, lateSubmit |-> FALSE, lateCommit |-> FALSE]

Tick ==
    /\ now < Deadline
    /\ now' = now + 1
    /\ UNCHANGED <<mode, phase, attempts, commit, effects, reply, evidence>>

CanRetry == now < Deadline /\ attempts < AttemptLimit

\* The native retry backoff may have started within budget and finished
\* at or after its deadline.
BeginAttempt ==
    /\ phase \in {"ready", "backoff"}
    /\ attempts < AttemptLimit
    /\ (now < Deadline \/ (phase = "backoff" /\ ~RecheckDeadline))
    /\ phase' = "callback"
    /\ attempts' = attempts + 1
    /\ evidence' = [evidence EXCEPT !.lateAttempt = @ \/ now >= Deadline]
    /\ UNCHANGED <<mode, now, commit, effects, reply>>

\* A callback can do local work or await a future before handing back
\* its tentative transaction. Its elapsed time counts toward admission.
SubmitCommit ==
    /\ phase = "callback"
    /\ (~RecheckDeadline \/ now < Deadline)
    /\ phase' = "commit"
    /\ commit' = "submitted"
    /\ evidence' = [evidence EXCEPT !.lateSubmit = @ \/ now >= Deadline]
    /\ UNCHANGED <<mode, now, attempts, effects, reply>>

DefiniteRetryableFailure ==
    /\ phase = "callback" \/ (phase = "commit" /\ commit = "submitted")
    /\ CanRetry
    /\ phase' = "backoff"
    /\ commit' = "none"
    /\ UNCHANGED <<mode, now, attempts, effects, reply, evidence>>

ReturnTimeout ==
    /\ phase # "returned"
    /\ now = Deadline
    /\ phase' = "returned"
    /\ reply' = "timeout"
    /\ UNCHANGED <<mode, now, attempts, commit, effects, evidence>>

\* A submitted mutation can settle after a timeout is reported. There is
\* no later attempt in this operation once a terminal error is returned.
SettleCommit ==
    /\ commit = "submitted"
    /\ commit' = "committed"
    /\ effects' = IF mode = "write" THEN effects + 1 ELSE effects
    /\ evidence' = [evidence EXCEPT !.lateCommit = @ \/ (mode = "write" /\ reply = "timeout")]
    /\ UNCHANGED <<mode, now, phase, attempts, reply>>

ObserveSuccess ==
    /\ phase = "commit"
    /\ commit = "committed"
    /\ phase' = "returned"
    /\ reply' = "success"
    /\ UNCHANGED <<mode, now, attempts, commit, effects, evidence>>

LostCommitReply ==
    /\ phase = "commit"
    /\ commit = "committed"
    /\ IF CanRetry /\ (mode = "read" \/ ~RejectAmbiguousWrite)
       THEN /\ phase' = "backoff"
            /\ commit' = "none"
            /\ UNCHANGED reply
       ELSE /\ phase' = "returned"
            /\ reply' = "unknown"
            /\ UNCHANGED commit
    /\ UNCHANGED <<mode, now, attempts, effects, evidence>>

Next ==
    \/ Tick \/ BeginAttempt \/ SubmitCommit \/ DefiniteRetryableFailure
    \/ ReturnTimeout \/ SettleCommit \/ ObserveSuccess \/ LostCommitReply

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ mode \in Modes
    /\ now \in 0..Deadline
    /\ phase \in Phases
    /\ attempts \in 0..AttemptLimit
    /\ commit \in {"none", "submitted", "committed"}
    /\ effects \in 0..AttemptLimit
    /\ reply \in {"none", "timeout", "unknown", "success"}
    /\ evidence \in [lateAttempt : BOOLEAN, lateSubmit : BOOLEAN, lateCommit : BOOLEAN]

NoAttemptAfterDeadline == ~evidence.lateAttempt
NoCommitSubmittedAfterDeadline == ~evidence.lateSubmit
NoDuplicateWrite == effects <= 1
NoLateSettlement == ~evidence.lateCommit

=============================================================================
