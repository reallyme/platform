// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral Hephaestus fleet-control domain types.

mod agent;
mod dns;
mod health;
mod inventory;
mod node;
mod region;
mod report;

pub use agent::{
    HephaestusAgentCapability, HephaestusAgentCapabilityCatalog, HephaestusAgentRolloutPhase,
    agent_capability_catalog,
};
pub use dns::{
    HephaestusDnsDesiredState, HephaestusDnsRecord, HephaestusDnsRecordKind,
    HephaestusDnsRecordName, HephaestusDnsRecordTarget, HephaestusDnsRecordVisibility,
};
pub use health::{
    HealthProbeStatus, HephaestusDomainError, HephaestusHealthCheckTarget, HephaestusHealthUrl,
    HephaestusServerHealthReport, HephaestusServerHost, HephaestusServerKitLogEvent,
    HephaestusServerKitLogEventCatalog, HephaestusServerPort, ProvisioningEventKind,
    ProvisioningEventSnapshot, server_kit_log_event_catalog,
};
pub use inventory::{HephaestusServerEnvironmentTag, HephaestusServerInventoryRow};
pub use node::{
    HephaestusAgentBootReport, HephaestusAgentBootReportResult, HephaestusAgentBootSecret,
    HephaestusAgentControlSecret, HephaestusAgentPublicKey, HephaestusAgentPublicKeyFingerprint,
    HephaestusAgentRuntimeToken, HephaestusClusterId, HephaestusControllerBaseUrl,
    HephaestusInfrastructureClassId, HephaestusMetadataBaseUrl, HephaestusNodeActualState,
    HephaestusNodeActualStateStatus, HephaestusOrchestratorNodeState, HephaestusPrivateIpAddress,
    HephaestusProviderServerId, HephaestusPublicIpAddress, HephaestusServerApp,
    HephaestusServerBootTiming, HephaestusServerCatalog, HephaestusServerId,
    HephaestusServerIdentity, HephaestusServerNetwork, HephaestusServerRole,
    HephaestusServerService,
};
pub use region::{HephaestusRegionDefinition, HephaestusRegionId, HephaestusSiteId};
pub use report::{
    HephaestusAgentReport, HephaestusAgentReportLine, HephaestusCadvisorSummary,
    HephaestusContainerHealthState, HephaestusContainerReport, HephaestusContainerRestartReason,
    HephaestusContainerState, HephaestusHostResourceReport, HephaestusHostServiceReport,
    HephaestusHostServiceState, HephaestusHostServiceUnitName, HephaestusNodeExporterSummary,
    HephaestusObservabilityReport, HephaestusObservedServiceSet, HephaestusPatchStateReport,
    HephaestusRuntimeVersionReport, HephaestusServiceProbeKind, HephaestusServiceProbeReport,
    HephaestusServiceProbeStatus, HephaestusTailscaleReport, HephaestusUnattendedUpgradesState,
};
