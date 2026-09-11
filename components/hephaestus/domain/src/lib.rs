// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! Host-neutral domain model owned by the Hephaestus application.
//!
//! The crate contains fleet-control and infrastructure-provider value types.
//! It intentionally has no dependency on protobuf, Connect, HTTP, application
//! hosts, or process runtimes. Wire conversions belong to the Hephaestus
//! contract crate.

/// Hephaestus fleet-control domain types.
pub mod hephaestus;
/// Infrastructure-provider types used by Hephaestus.
pub mod vultr;

pub use hephaestus::{
    HealthProbeStatus, HephaestusAgentBootReport, HephaestusAgentBootReportResult,
    HephaestusAgentBootSecret, HephaestusAgentCapability, HephaestusAgentCapabilityCatalog,
    HephaestusAgentControlSecret, HephaestusAgentPublicKey, HephaestusAgentPublicKeyFingerprint,
    HephaestusAgentReport, HephaestusAgentReportLine, HephaestusAgentRolloutPhase,
    HephaestusAgentRuntimeToken, HephaestusCadvisorSummary, HephaestusClusterId,
    HephaestusContainerHealthState, HephaestusContainerReport, HephaestusContainerRestartReason,
    HephaestusContainerState, HephaestusControllerBaseUrl, HephaestusDnsDesiredState,
    HephaestusDnsRecord, HephaestusDnsRecordKind, HephaestusDnsRecordName,
    HephaestusDnsRecordTarget, HephaestusDnsRecordVisibility, HephaestusDomainError,
    HephaestusHealthCheckTarget, HephaestusHealthUrl, HephaestusHostResourceReport,
    HephaestusHostServiceReport, HephaestusHostServiceState, HephaestusHostServiceUnitName,
    HephaestusInfrastructureClassId, HephaestusMetadataBaseUrl, HephaestusNodeActualState,
    HephaestusNodeActualStateStatus, HephaestusNodeExporterSummary, HephaestusObservabilityReport,
    HephaestusObservedServiceSet, HephaestusOrchestratorNodeState, HephaestusPatchStateReport,
    HephaestusPrivateIpAddress, HephaestusProviderServerId, HephaestusPublicIpAddress,
    HephaestusRegionDefinition, HephaestusRegionId, HephaestusRuntimeVersionReport,
    HephaestusServerApp, HephaestusServerBootTiming, HephaestusServerCatalog,
    HephaestusServerEnvironmentTag, HephaestusServerHealthReport, HephaestusServerHost,
    HephaestusServerId, HephaestusServerIdentity, HephaestusServerInventoryRow,
    HephaestusServerKitLogEvent, HephaestusServerKitLogEventCatalog, HephaestusServerNetwork,
    HephaestusServerPort, HephaestusServerRole, HephaestusServerService,
    HephaestusServiceProbeKind, HephaestusServiceProbeReport, HephaestusServiceProbeStatus,
    HephaestusSiteId, HephaestusTailscaleReport, HephaestusUnattendedUpgradesState,
    ProvisioningEventKind, ProvisioningEventSnapshot, agent_capability_catalog,
    server_kit_log_event_catalog,
};
pub use vultr::{
    DockerCloudInitSpec, DockerContainerName, DockerDnsResolver, DockerImageReference,
    DockerRestartPolicy, DockerRunArgument, DockerVolumeMount, HephaestusAgentCloudInitSpec,
    HephaestusServerMetadata, SshAuthorizedKey, VultrCloudInitUserData, VultrContainerArtifact,
    VultrContainerArtifactDigest, VultrContainerRegistry, VultrContainerRegistryId,
    VultrContainerRegistryName, VultrContainerRegistryUrn, VultrContainerRepository,
    VultrContainerRepositoryName, VultrCountryCode, VultrDatacenterCode, VultrError,
    VultrFirewallGroupId, VultrHostname, VultrInstance, VultrInstanceId, VultrInstanceNetwork,
    VultrInstanceStatus, VultrInstanceTemplate, VultrInstanceTemplateId, VultrIsoId, VultrLabel,
    VultrMarketplaceAppId, VultrMarketplaceImageId, VultrOperatingSystemId, VultrPlan, VultrPlanId,
    VultrPlanType, VultrPlanTypeCode, VultrPrivateIpAddress, VultrPublicIpAddress, VultrRegion,
    VultrRegionId, VultrSnapshotId, VultrSshKeyId, VultrStartupScriptId, VultrTag, VultrVfsId,
    VultrVpc, VultrVpcId,
};
