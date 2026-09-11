// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral Vultr domain types.
//!
//! These types intentionally model Vultr concepts without depending on Vultr's
//! HTTP client, app runtime, transport adapters, databases, or process
//! lifecycle. They are shared by the Hephaestus app contract and adapters.

mod bootstrap;
mod error;
mod types;

pub use bootstrap::{
    DockerCloudInitSpec, DockerContainerName, DockerDnsResolver, DockerImageReference,
    DockerRestartPolicy, DockerRunArgument, DockerVolumeMount, HephaestusAgentCloudInitSpec,
    HephaestusServerMetadata, SshAuthorizedKey,
};
pub use error::VultrError;
pub use types::{
    VultrCloudInitUserData, VultrContainerArtifact, VultrContainerArtifactDigest,
    VultrContainerRegistry, VultrContainerRegistryId, VultrContainerRegistryName,
    VultrContainerRegistryUrn, VultrContainerRepository, VultrContainerRepositoryName,
    VultrCountryCode, VultrDatacenterCode, VultrFirewallGroupId, VultrHostname, VultrInstance,
    VultrInstanceId, VultrInstanceNetwork, VultrInstanceStatus, VultrInstanceTemplate,
    VultrInstanceTemplateId, VultrIsoId, VultrLabel, VultrMarketplaceAppId,
    VultrMarketplaceImageId, VultrOperatingSystemId, VultrPlan, VultrPlanId, VultrPlanType,
    VultrPlanTypeCode, VultrPrivateIpAddress, VultrPublicIpAddress, VultrRegion, VultrRegionId,
    VultrSnapshotId, VultrSshKeyId, VultrStartupScriptId, VultrTag, VultrVfsId, VultrVpc,
    VultrVpcId,
};
