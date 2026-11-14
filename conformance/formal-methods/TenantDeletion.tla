---------------------------- MODULE TenantDeletion ----------------------------
\* SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
\* SPDX-License-Identifier: MIT OR Apache-2.0

VARIABLES tenantExists, appData, metadata, savedMetadata, phase, deleteResult, recovered

vars == <<tenantExists, appData, metadata, savedMetadata, phase, deleteResult, recovered>>
KitMetadata == {"schema", "createdAt"}
MetadataKeys == KitMetadata \cup {"foreign"}
ActivePhases == {"start", "checked", "cleared", "restore"}

Init ==
    /\ tenantExists = TRUE
    /\ appData \in BOOLEAN
    /\ metadata \in SUBSET MetadataKeys
    /\ savedMetadata = {}
    /\ phase = "start"
    /\ deleteResult = "pending"
    /\ recovered = FALSE

\* An already-open tenant handle can commit a write even while metadata is absent.
WriteApplicationData ==
    /\ tenantExists
    /\ ~appData
    /\ appData' = TRUE
    /\ UNCHANGED <<tenantExists, metadata, savedMetadata, phase, deleteResult, recovered>>

ReadEmptyTenant ==
    /\ phase = "start"
    /\ ~appData
    /\ metadata \subseteq KitMetadata
    /\ savedMetadata' = metadata
    /\ phase' = "checked"
    /\ UNCHANGED <<tenantExists, appData, metadata, deleteResult, recovered>>

\* The phase-one range read is non-snapshot. A concurrent write to that range
\* prevents the clear from committing against the old read.
CommitMetadataClear ==
    /\ phase = "checked"
    /\ ~appData
    /\ metadata' = {}
    /\ phase' \in {"cleared", "restore"}
    /\ UNCHANGED <<tenantExists, appData, savedMetadata, deleteResult, recovered>>

\* A failed or uncertain clear can leave the metadata intact.
FailMetadataClear ==
    /\ phase = "checked"
    /\ phase' = "restore"
    /\ UNCHANGED <<tenantExists, appData, metadata, savedMetadata, deleteResult, recovered>>

RejectNonemptyTenant ==
    /\ phase = "start"
    /\ (appData \/ ~(metadata \subseteq KitMetadata))
    /\ phase' = "finished"
    /\ deleteResult' = "failed"
    /\ UNCHANGED <<tenantExists, appData, metadata, savedMetadata, recovered>>

\* FoundationDB independently checks the whole tenant at the second commit.
\* A successful commit can be acknowledged directly or observed by lookup.
CommitTenantDelete ==
    /\ phase = "cleared"
    /\ ~appData
    /\ metadata = {}
    /\ tenantExists' = FALSE
    /\ phase' \in {"deleted", "finished"}
    /\ deleteResult' = IF phase' = "finished" THEN "deleted" ELSE "pending"
    /\ UNCHANGED <<appData, metadata, savedMetadata, recovered>>

ObserveTenantAbsent ==
    /\ phase = "deleted"
    /\ phase' = "finished"
    /\ deleteResult' = "deleted"
    /\ UNCHANGED <<tenantExists, appData, metadata, savedMetadata, recovered>>

FailTenantDelete ==
    /\ phase = "cleared"
    /\ phase' = "restore"
    /\ UNCHANGED <<tenantExists, appData, metadata, savedMetadata, deleteResult, recovered>>

\* Collapse the bounded restore attempts into success or exhaustion.
FinishRestore(succeeds) ==
    /\ phase = "restore"
    /\ metadata' = IF succeeds THEN metadata \cup savedMetadata ELSE metadata
    /\ phase' = "finished"
    /\ deleteResult' = "failed"
    /\ UNCHANGED <<tenantExists, appData, savedMetadata, recovered>>

Interrupt ==
    /\ phase \in ActivePhases \cup {"deleted"}
    /\ phase' = "interrupted"
    /\ UNCHANGED <<tenantExists, appData, metadata, savedMetadata, deleteResult, recovered>>

\* Recovery can initialize the reserved namespace of the recorded tenant
\* after an interrupted or failed operation, including when app data exists.
RecoverMetadata ==
    /\ phase \in {"finished", "interrupted"}
    /\ tenantExists
    /\ metadata = {}
    /\ metadata' = KitMetadata
    /\ recovered' = TRUE
    /\ UNCHANGED <<tenantExists, appData, savedMetadata, phase, deleteResult>>

Quiesce ==
    /\ phase \in {"finished", "interrupted"}
    /\ UNCHANGED vars

Next ==
    \/ WriteApplicationData
    \/ ReadEmptyTenant
    \/ CommitMetadataClear
    \/ FailMetadataClear
    \/ RejectNonemptyTenant
    \/ CommitTenantDelete
    \/ ObserveTenantAbsent
    \/ FailTenantDelete
    \/ \E succeeds \in BOOLEAN : FinishRestore(succeeds)
    \/ Interrupt
    \/ RecoverMetadata
    \/ Quiesce

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ tenantExists \in BOOLEAN
    /\ appData \in BOOLEAN
    /\ metadata \in SUBSET MetadataKeys
    /\ savedMetadata \in SUBSET KitMetadata
    /\ phase \in ActivePhases \cup {"deleted", "finished", "interrupted"}
    /\ deleteResult \in {"pending", "failed", "deleted"}
    /\ recovered \in BOOLEAN

ApplicationDataSurvives == appData => tenantExists
DeletedTenantHasNoMetadata == ~tenantExists => metadata = {}
DeleteSuccessRequiresAbsence == deleteResult = "deleted" => ~tenantExists
RecoveryRestoresBothKeys == recovered => KitMetadata \subseteq metadata

PreserveUnownedMetadata == [][("foreign" \in metadata => "foreign" \in metadata')]_vars
PreserveApplicationData == [][(appData => appData')]_vars
RecoveryOnlyInitializesAbsentMetadata == [][(~recovered /\ recovered' => metadata = {})]_vars

=============================================================================
