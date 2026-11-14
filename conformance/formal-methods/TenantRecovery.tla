---------------------------- MODULE TenantRecovery ----------------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

EXTENDS FiniteSets

CONSTANTS RecordedId, ReplacementId, NoTenant, CheckBoundId

ASSUME /\ Cardinality({RecordedId, ReplacementId, NoTenant}) = 3
       /\ CheckBoundId \in BOOLEAN

VARIABLES nameId, metadata, phase, boundId, writtenId, outcome

vars == <<nameId, metadata, phase, boundId, writtenId, outcome>>
TenantIds == {RecordedId, ReplacementId}
KitMetadata == {"schema", "createdAt"}
MetadataKeys == KitMetadata \cup {"foreign"}
PreBinding == {"start", "checked", "opened"}
InFlight == {"prepared", "timed-out"}

Init ==
    /\ nameId = RecordedId
    /\ metadata = {}
    /\ phase = "start"
    /\ boundId = NoTenant
    /\ writtenId = NoTenant
    /\ outcome = "pending"

\* Start after the interrupted metadata clear. Reuse requires an empty
\* original tenant and allocates a distinct ID for the same name.
DeleteOriginal ==
    /\ nameId = RecordedId
    /\ metadata = {}
    /\ nameId' = NoTenant
    /\ UNCHANGED <<metadata, phase, boundId, writtenId, outcome>>

ReuseName ==
    /\ nameId = NoTenant
    /\ nameId' = ReplacementId
    /\ UNCHANGED <<metadata, phase, boundId, writtenId, outcome>>

\* Provisioning or another operator can populate any part of the reserved
\* namespace. A non-snapshot recovery read conflicts with a later write.
PopulateMetadata ==
    /\ nameId \in TenantIds
    /\ metadata = {}
    /\ metadata' \in (SUBSET MetadataKeys) \ {{}}
    /\ UNCHANGED <<nameId, phase, boundId, writtenId, outcome>>

CheckRecordedId ==
    /\ phase = "start"
    /\ nameId = RecordedId
    /\ phase' = "checked"
    /\ UNCHANGED <<nameId, metadata, boundId, writtenId, outcome>>

\* Opening by name does not convey the result of the management lookup.
\* Resolve the transaction's keyspace separately, at its metadata read.
OpenByName ==
    /\ phase = "checked"
    /\ phase' = "opened"
    /\ UNCHANGED <<nameId, metadata, boundId, writtenId, outcome>>

ReadEmptyMetadata ==
    /\ phase = "opened"
    /\ nameId \in TenantIds
    /\ metadata = {}
    /\ (~CheckBoundId \/ nameId = RecordedId)
    /\ boundId' = nameId
    /\ phase' = "prepared"
    /\ UNCHANGED <<nameId, metadata, writtenId, outcome>>

RejectBeforeWrite ==
    /\ phase \in PreBinding
    /\ phase' = "done"
    /\ outcome' = "rejected"
    /\ UNCHANGED <<nameId, metadata, boundId, writtenId>>

\* Losing the response cannot tell the caller whether the submitted
\* transaction will commit. Keep its target after a timeout.
ReportTimeout ==
    /\ phase = "prepared"
    /\ phase' = "timed-out"
    /\ outcome' = "timeout"
    /\ UNCHANGED <<nameId, metadata, boundId, writtenId>>

CommitRecovery ==
    /\ phase \in InFlight
    /\ boundId = nameId
    /\ metadata = {}
    /\ metadata' = KitMetadata
    /\ writtenId' = boundId
    /\ phase' = "done"
    /\ outcome' = IF phase = "timed-out" THEN "timeout" ELSE "success"
    /\ UNCHANGED <<nameId, boundId>>

AbortRecovery ==
    /\ phase \in InFlight
    /\ phase' = "done"
    /\ outcome' = IF phase = "timed-out" THEN "timeout" ELSE "rejected"
    /\ UNCHANGED <<nameId, metadata, boundId, writtenId>>

Quiesce == phase = "done" /\ UNCHANGED vars

Next ==
    \/ DeleteOriginal
    \/ ReuseName
    \/ PopulateMetadata
    \/ CheckRecordedId
    \/ OpenByName
    \/ ReadEmptyMetadata
    \/ RejectBeforeWrite
    \/ ReportTimeout
    \/ CommitRecovery
    \/ AbortRecovery
    \/ Quiesce

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ nameId \in TenantIds \cup {NoTenant}
    /\ metadata \in SUBSET MetadataKeys
    /\ phase \in PreBinding \cup InFlight \cup {"done"}
    /\ boundId \in TenantIds \cup {NoTenant}
    /\ writtenId \in TenantIds \cup {NoTenant}
    /\ outcome \in {"pending", "rejected", "timeout", "success"}

RecoveryWritesOnlyRecordedTenant == writtenId \in {NoTenant, RecordedId}
SuccessRequiresCommit == outcome = "success" => writtenId \in TenantIds
RecoveryWritesBothKeys == writtenId \in TenantIds => KitMetadata \subseteq metadata
NoMetadataWithoutTenant == nameId = NoTenant => metadata = {}

RecoveryOnlyInitializesEmptyMetadata ==
    [][(writtenId' # writtenId => metadata = {})]_vars

=============================================================================
