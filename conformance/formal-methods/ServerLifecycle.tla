----------------------------- MODULE ServerLifecycle -----------------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

CONSTANTS HandleSignalError, WaitForPropagation

Listeners == {"http", "grpc"}
Tasks == {"storage", "worker"}
Reasons == {"signal", "signal-error", "critical-error", "startup-error"}
Phases == {"starting", "serving", "propagating", "draining", "cleaning", "finished"}
Resolved == {"stopped", "abort-requested"}

VARIABLES phase, ready, wasReady, listeners, reported, reason, propagated

vars == <<phase, ready, wasReady, listeners, reported, reason, propagated>>

Init ==
    /\ phase = "starting"
    /\ ready = FALSE
    /\ wasReady = FALSE
    /\ listeners = [listener \in Listeners |-> "pending"]
    /\ reported = {}
    /\ reason = "none"
    /\ propagated = FALSE

StartListener(listener) ==
    /\ phase = "starting"
    /\ listeners[listener] = "pending"
    /\ listeners' = [listeners EXCEPT ![listener] = "listening"]
    /\ UNCHANGED <<phase, ready, wasReady, reported, reason, propagated>>

ReportTaskReady(task) ==
    /\ phase = "starting"
    /\ \A listener \in Listeners : listeners[listener] = "listening"
    /\ task \notin reported
    /\ reported' = reported \cup {task}
    /\ UNCHANGED <<phase, ready, wasReady, listeners, reason, propagated>>

PublishReady ==
    /\ phase = "starting"
    /\ \A listener \in Listeners : listeners[listener] = "listening"
    /\ reported = Tasks
    /\ phase' = "serving"
    /\ ready' = TRUE
    /\ wasReady' = TRUE
    /\ UNCHANGED <<listeners, reported, reason, propagated>>

\* An installation failure occurs before readiness. Already started
\* listeners can be shut down without a propagation interval.
StartupError ==
    /\ phase = "starting"
    /\ phase' = "draining"
    /\ reason' = "startup-error"
    /\ UNCHANGED <<ready, wasReady, listeners, reported, propagated>>

\* Models observation by the supervisor. A running task can exit before its
\* failure is observed. The readiness transition occurs once it is observed.
StartShutdown(cause) ==
    /\ (phase = "serving" \/ (phase = "starting" /\ cause = "critical-error"))
    /\ reason' = cause
    /\ IF cause = "signal-error" /\ ~HandleSignalError
       THEN /\ phase' = "finished"
            /\ UNCHANGED ready
       ELSE /\ phase' = "propagating"
            /\ ready' = FALSE
    /\ UNCHANGED <<wasReady, listeners, reported, propagated>>

ObservePropagation ==
    /\ phase = "propagating"
    /\ ~propagated
    /\ propagated' = TRUE
    /\ UNCHANGED <<phase, ready, wasReady, listeners, reported, reason>>

StartDraining ==
    /\ phase = "propagating"
    /\ propagated \/ ~WaitForPropagation
    /\ phase' = "draining"
    /\ UNCHANGED <<ready, wasReady, listeners, reported, reason, propagated>>

\* The drain resolves each listener by observing its stop or requesting
\* cancellation at the deadline. Cancellation can still be settling.
ResolveListener(listener, disposition) ==
    /\ phase = "draining"
    /\ listeners[listener] \notin Resolved
    /\ listeners' = [listeners EXCEPT ![listener] = disposition]
    /\ UNCHANGED <<phase, ready, wasReady, reported, reason, propagated>>

AllListenersResolved == \A listener \in Listeners : listeners[listener] \in Resolved

StartCleanup ==
    /\ phase = "draining"
    /\ AllListenersResolved
    /\ phase' = "cleaning"
    /\ UNCHANGED <<ready, wasReady, listeners, reported, reason, propagated>>

Finish ==
    /\ phase = "cleaning"
    /\ phase' = "finished"
    /\ UNCHANGED <<ready, wasReady, listeners, reported, reason, propagated>>

Next ==
    \/ \E listener \in Listeners :
          \/ StartListener(listener)
          \/ \E disposition \in Resolved : ResolveListener(listener, disposition)
    \/ \E task \in Tasks : ReportTaskReady(task)
    \/ \E cause \in Reasons \ {"startup-error"} : StartShutdown(cause)
    \/ PublishReady \/ StartupError \/ ObservePropagation \/ StartDraining \/ StartCleanup \/ Finish

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ phase \in Phases
    /\ ready \in BOOLEAN
    /\ wasReady \in BOOLEAN
    /\ listeners \in [Listeners -> {"pending", "listening"} \cup Resolved]
    /\ reported \subseteq Tasks
    /\ reason \in Reasons \cup {"none"}
    /\ propagated \in BOOLEAN

ReadyRequiresStartup ==
    ready =>
        /\ reported = Tasks
        /\ \A listener \in Listeners : listeners[listener] = "listening"

ReadinessWithdrawnDuringShutdown == reason \in Reasons => ~ready
ReadyNeverReturns == [][(reason \in Reasons => ~ready')]_vars

ServingDrainWaitsForPropagation ==
    (wasReady /\ phase \in {"draining", "cleaning", "finished"}) => propagated

CleanupFollowsDrain == phase \in {"cleaning", "finished"} => AllListenersResolved

\* Require the signal watcher failure path to reach the end of shutdown.
NoOrderedSignalErrorShutdown ==
    ~(/\ reason = "signal-error"
      /\ phase = "finished"
      /\ AllListenersResolved
      /\ propagated
      /\ \E listener \in Listeners : listeners[listener] = "abort-requested")

=============================================================================
