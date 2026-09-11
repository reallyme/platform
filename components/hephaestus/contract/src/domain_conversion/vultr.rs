// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Conversion between Vultr domain DTOs and authoritative domain types.

use buffa::Enumeration as _;

use crate::generated::proto::reallyme::domain::v1 as proto;

use reallyme_hephaestus_domain::{
    VultrCloudInitUserData, VultrContainerArtifact, VultrContainerArtifactDigest,
    VultrContainerRegistry, VultrContainerRegistryId, VultrContainerRegistryName,
    VultrContainerRegistryUrn, VultrContainerRepository, VultrContainerRepositoryName,
    VultrCountryCode, VultrError, VultrInstance, VultrInstanceId, VultrInstanceNetwork,
    VultrInstanceStatus, VultrInstanceTemplate, VultrInstanceTemplateId, VultrIsoId, VultrLabel,
    VultrMarketplaceAppId, VultrMarketplaceImageId, VultrOperatingSystemId, VultrPlan, VultrPlanId,
    VultrPlanType, VultrPrivateIpAddress, VultrPublicIpAddress, VultrRegion, VultrRegionId,
    VultrSnapshotId, VultrSshKeyId, VultrStartupScriptId, VultrTag, VultrVfsId, VultrVpc,
    VultrVpcId,
};

/// Converts a protobuf Vultr region DTO into a validated domain type.
pub fn vultr_region_from_proto(value: proto::VultrRegion) -> Result<VultrRegion, VultrError> {
    VultrRegion::new(
        VultrRegionId::new(value.id)?,
        value.city,
        VultrCountryCode::new(value.country)?,
        value.continent,
    )
}

/// Converts a domain Vultr region into a protobuf DTO.
pub fn proto_vultr_region_from_domain(value: &VultrRegion) -> proto::VultrRegion {
    proto::VultrRegion {
        id: value.id().as_str().to_owned(),
        city: value.city().to_owned(),
        country: value.country().as_str().to_owned(),
        continent: value.continent().to_owned(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf Vultr plan DTO into a validated domain type.
pub fn vultr_plan_from_proto(value: proto::VultrPlan) -> Result<VultrPlan, VultrError> {
    VultrPlan::new(
        VultrPlanId::new(value.id)?,
        vultr_plan_type_from_proto_i32(value.plan_type.to_i32())?,
        value.vcpu_count,
        value.ram_mb,
        value.disk_gb,
        value.monthly_cost_cents,
    )
}

/// Converts a domain Vultr plan into a protobuf DTO.
pub fn proto_vultr_plan_from_domain(value: &VultrPlan) -> proto::VultrPlan {
    proto::VultrPlan {
        id: value.id().as_str().to_owned(),
        plan_type: buffa::EnumValue::from(
            proto_vultr_plan_type_from_domain(value.plan_type()) as i32
        ),
        vcpu_count: value.vcpu_count(),
        ram_mb: value.ram_mb(),
        disk_gb: value.disk_gb(),
        monthly_cost_cents: value.monthly_cost_cents(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf Vultr VPC DTO into a validated domain type.
pub fn vultr_vpc_from_proto(value: proto::VultrVpc) -> Result<VultrVpc, VultrError> {
    VultrVpc::new(
        VultrVpcId::new(value.id)?,
        VultrRegionId::new(value.region)?,
        value.description,
        value.ip_block,
        u8::try_from(value.prefix_length).map_err(|_error| VultrError::InvalidNumber)?,
    )
}

/// Converts a domain Vultr VPC into a protobuf DTO.
pub fn proto_vultr_vpc_from_domain(value: &VultrVpc) -> proto::VultrVpc {
    proto::VultrVpc {
        id: value.id().as_str().to_owned(),
        region: value.region().as_str().to_owned(),
        description: value.description().to_owned(),
        ip_block: value.ip_block().to_owned(),
        prefix_length: u32::from(value.prefix_length()),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf Vultr instance DTO into a validated domain type.
pub fn vultr_instance_from_proto(value: proto::VultrInstance) -> Result<VultrInstance, VultrError> {
    let main_ip = empty_string_as_none(value.main_ip)
        .map(VultrPublicIpAddress::new)
        .transpose()?;
    let internal_ip = empty_string_as_none(value.internal_ip)
        .map(VultrPrivateIpAddress::new)
        .transpose()?;
    let vpc_ids = value
        .vpc_ids
        .into_iter()
        .map(VultrVpcId::new)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(VultrInstance::new(
        VultrInstanceId::new(value.id)?,
        VultrRegionId::new(value.region)?,
        VultrPlanId::new(value.plan)?,
        VultrLabel::new(value.label)?,
        vultr_instance_status_from_proto_i32(value.status.to_i32())?,
        VultrInstanceNetwork::new(main_ip, internal_ip, vpc_ids, value.vpc_only),
    ))
}

/// Converts a domain Vultr instance into a protobuf DTO.
pub fn proto_vultr_instance_from_domain(value: &VultrInstance) -> proto::VultrInstance {
    proto::VultrInstance {
        id: value.id().as_str().to_owned(),
        region: value.region().as_str().to_owned(),
        plan: value.plan().as_str().to_owned(),
        label: value.label().as_str().to_owned(),
        status: buffa::EnumValue::from(
            proto_vultr_instance_status_from_domain(value.status()) as i32
        ),
        main_ip: value
            .main_ip()
            .map(VultrPublicIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        internal_ip: value
            .internal_ip()
            .map(VultrPrivateIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        vpc_ids: value
            .vpc_ids()
            .iter()
            .map(VultrVpcId::as_str)
            .map(str::to_owned)
            .collect(),
        vpc_only: value.vpc_only(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf Vultr instance-template DTO into a validated domain type.
pub fn vultr_instance_template_from_proto(
    value: proto::VultrInstanceTemplate,
) -> Result<VultrInstanceTemplate, VultrError> {
    let label = empty_string_as_none(value.label)
        .map(VultrLabel::new)
        .transpose()?;
    let os_id = if value.os_id == 0 {
        None
    } else {
        Some(VultrOperatingSystemId::new(value.os_id)?)
    };
    let iso_id = empty_string_as_none(value.iso_id)
        .map(VultrIsoId::new)
        .transpose()?;
    let snapshot_id = empty_string_as_none(value.snapshot_id)
        .map(VultrSnapshotId::new)
        .transpose()?;
    let marketplace_app_id = if value.marketplace_app_id == 0 {
        None
    } else {
        Some(VultrMarketplaceAppId::new(value.marketplace_app_id)?)
    };
    let marketplace_image_id = if value.marketplace_image_id == 0 {
        None
    } else {
        Some(VultrMarketplaceImageId::new(value.marketplace_image_id)?)
    };
    let script_id = empty_string_as_none(value.script_id)
        .map(VultrStartupScriptId::new)
        .transpose()?;
    let ssh_key_ids = value
        .ssh_key_ids
        .into_iter()
        .map(VultrSshKeyId::new)
        .collect::<Result<Vec<_>, _>>()?;
    let vpc_ids = value
        .vpc_ids
        .into_iter()
        .map(VultrVpcId::new)
        .collect::<Result<Vec<_>, _>>()?;
    let vfs_ids = value
        .vfs_ids
        .into_iter()
        .map(VultrVfsId::new)
        .collect::<Result<Vec<_>, _>>()?;
    let user_data = empty_string_as_none(value.user_data)
        .map(VultrCloudInitUserData::new)
        .transpose()?;

    Ok(VultrInstanceTemplate::new(
        VultrInstanceTemplateId::new(value.id)?,
        VultrPlanId::new(value.plan)?,
        label,
        os_id,
        iso_id,
        snapshot_id,
        marketplace_app_id,
        marketplace_image_id,
        script_id,
        ssh_key_ids,
        vpc_ids,
        vfs_ids,
        user_data,
    ))
}

/// Converts a domain Vultr instance-template into a protobuf DTO.
pub fn proto_vultr_instance_template_from_domain(
    value: &VultrInstanceTemplate,
) -> proto::VultrInstanceTemplate {
    proto::VultrInstanceTemplate {
        id: value.id().as_str().to_owned(),
        plan: value.plan().as_str().to_owned(),
        label: value
            .label()
            .map(VultrLabel::as_str)
            .unwrap_or_default()
            .to_owned(),
        os_id: value.os_id().map(VultrOperatingSystemId::get).unwrap_or(0),
        iso_id: value
            .iso_id()
            .map(VultrIsoId::as_str)
            .unwrap_or_default()
            .to_owned(),
        snapshot_id: value
            .snapshot_id()
            .map(VultrSnapshotId::as_str)
            .unwrap_or_default()
            .to_owned(),
        marketplace_app_id: value
            .marketplace_app_id()
            .map(VultrMarketplaceAppId::get)
            .unwrap_or(0),
        marketplace_image_id: value
            .marketplace_image_id()
            .map(VultrMarketplaceImageId::get)
            .unwrap_or(0),
        script_id: value
            .script_id()
            .map(VultrStartupScriptId::as_str)
            .unwrap_or_default()
            .to_owned(),
        ssh_key_ids: value
            .ssh_key_ids()
            .iter()
            .map(VultrSshKeyId::as_str)
            .map(str::to_owned)
            .collect(),
        vpc_ids: value
            .vpc_ids()
            .iter()
            .map(VultrVpcId::as_str)
            .map(str::to_owned)
            .collect(),
        vfs_ids: value
            .vfs_ids()
            .iter()
            .map(VultrVfsId::as_str)
            .map(str::to_owned)
            .collect(),
        user_data: value
            .user_data()
            .map(VultrCloudInitUserData::expose_as_str)
            .unwrap_or_default()
            .to_owned(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf Vultr container-registry DTO into a validated domain type.
pub fn vultr_container_registry_from_proto(
    value: proto::VultrContainerRegistry,
) -> Result<VultrContainerRegistry, VultrError> {
    Ok(VultrContainerRegistry::new(
        VultrContainerRegistryId::new(value.id)?,
        VultrContainerRegistryName::new(value.name)?,
        value.public,
        VultrContainerRegistryUrn::new(value.urn)?,
        value.storage_bytes_used,
    ))
}

/// Converts a domain Vultr container-registry into a protobuf DTO.
pub fn proto_vultr_container_registry_from_domain(
    value: &VultrContainerRegistry,
) -> proto::VultrContainerRegistry {
    proto::VultrContainerRegistry {
        id: value.id().as_str().to_owned(),
        name: value.name().as_str().to_owned(),
        public: value.public(),
        urn: value.urn().as_str().to_owned(),
        storage_bytes_used: value.storage_bytes_used(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf Vultr container-repository DTO into a validated domain type.
pub fn vultr_container_repository_from_proto(
    value: proto::VultrContainerRepository,
) -> Result<VultrContainerRepository, VultrError> {
    VultrContainerRepository::new(
        VultrContainerRegistryId::new(value.registry_id)?,
        VultrContainerRepositoryName::new(value.name)?,
        value.description,
        value.pull_count,
        value.artifact_count,
    )
}

/// Converts a domain Vultr container-repository into a protobuf DTO.
pub fn proto_vultr_container_repository_from_domain(
    value: &VultrContainerRepository,
) -> proto::VultrContainerRepository {
    proto::VultrContainerRepository {
        registry_id: value.registry_id().as_str().to_owned(),
        name: value.name().as_str().to_owned(),
        description: value.description().to_owned(),
        pull_count: value.pull_count(),
        artifact_count: value.artifact_count(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf Vultr artifact DTO into a validated domain type.
pub fn vultr_container_artifact_from_proto(
    value: proto::VultrContainerArtifact,
) -> Result<VultrContainerArtifact, VultrError> {
    let tags = value
        .tags
        .into_iter()
        .map(VultrTag::new)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(VultrContainerArtifact::new(
        VultrContainerRegistryId::new(value.registry_id)?,
        VultrContainerRepositoryName::new(value.repository)?,
        VultrContainerArtifactDigest::new(value.digest)?,
        tags,
        value.size_bytes,
    ))
}

/// Converts a domain Vultr artifact into a protobuf DTO.
pub fn proto_vultr_container_artifact_from_domain(
    value: &VultrContainerArtifact,
) -> proto::VultrContainerArtifact {
    proto::VultrContainerArtifact {
        registry_id: value.registry_id().as_str().to_owned(),
        repository: value.repository().as_str().to_owned(),
        digest: value.digest().as_str().to_owned(),
        tags: value
            .tags()
            .iter()
            .map(VultrTag::as_str)
            .map(str::to_owned)
            .collect(),
        size_bytes: value.size_bytes(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a raw protobuf plan-type value into a domain enum.
pub fn vultr_plan_type_from_proto_i32(value: i32) -> Result<VultrPlanType, VultrError> {
    match proto::VultrPlanType::from_i32(value).ok_or(VultrError::UnknownPlanType)? {
        proto::VultrPlanType::VULTR_PLAN_TYPE_CLOUD_COMPUTE => Ok(VultrPlanType::CloudCompute),
        proto::VultrPlanType::VULTR_PLAN_TYPE_DEDICATED_CLOUD => Ok(VultrPlanType::DedicatedCloud),
        proto::VultrPlanType::VULTR_PLAN_TYPE_HIGH_FREQUENCY => Ok(VultrPlanType::HighFrequency),
        proto::VultrPlanType::VULTR_PLAN_TYPE_HIGH_PERFORMANCE => {
            Ok(VultrPlanType::HighPerformance)
        }
        proto::VultrPlanType::VULTR_PLAN_TYPE_OPTIMIZED_CLOUD => Ok(VultrPlanType::OptimizedCloud),
        proto::VultrPlanType::VULTR_PLAN_TYPE_BARE_METAL => Ok(VultrPlanType::BareMetal),
        proto::VultrPlanType::VULTR_PLAN_TYPE_CLOUD_GPU => Ok(VultrPlanType::CloudGpu),
        proto::VultrPlanType::VULTR_PLAN_TYPE_UNSPECIFIED => Err(VultrError::UnknownPlanType),
    }
}

/// Converts a domain plan-type enum into a protobuf enum.
pub const fn proto_vultr_plan_type_from_domain(value: VultrPlanType) -> proto::VultrPlanType {
    match value {
        VultrPlanType::CloudCompute => proto::VultrPlanType::VULTR_PLAN_TYPE_CLOUD_COMPUTE,
        VultrPlanType::DedicatedCloud => proto::VultrPlanType::VULTR_PLAN_TYPE_DEDICATED_CLOUD,
        VultrPlanType::HighFrequency => proto::VultrPlanType::VULTR_PLAN_TYPE_HIGH_FREQUENCY,
        VultrPlanType::HighPerformance => proto::VultrPlanType::VULTR_PLAN_TYPE_HIGH_PERFORMANCE,
        VultrPlanType::OptimizedCloud => proto::VultrPlanType::VULTR_PLAN_TYPE_OPTIMIZED_CLOUD,
        VultrPlanType::BareMetal => proto::VultrPlanType::VULTR_PLAN_TYPE_BARE_METAL,
        VultrPlanType::CloudGpu => proto::VultrPlanType::VULTR_PLAN_TYPE_CLOUD_GPU,
    }
}

/// Converts a raw protobuf instance-status value into a domain enum.
pub fn vultr_instance_status_from_proto_i32(value: i32) -> Result<VultrInstanceStatus, VultrError> {
    match proto::VultrInstanceStatus::from_i32(value).ok_or(VultrError::UnknownInstanceStatus)? {
        proto::VultrInstanceStatus::VULTR_INSTANCE_STATUS_PENDING => {
            Ok(VultrInstanceStatus::Pending)
        }
        proto::VultrInstanceStatus::VULTR_INSTANCE_STATUS_ACTIVE => Ok(VultrInstanceStatus::Active),
        proto::VultrInstanceStatus::VULTR_INSTANCE_STATUS_STOPPED => {
            Ok(VultrInstanceStatus::Stopped)
        }
        proto::VultrInstanceStatus::VULTR_INSTANCE_STATUS_SUSPENDED => {
            Ok(VultrInstanceStatus::Suspended)
        }
        proto::VultrInstanceStatus::VULTR_INSTANCE_STATUS_UNSPECIFIED => {
            Err(VultrError::UnknownInstanceStatus)
        }
    }
}

/// Converts a domain instance-status enum into a protobuf enum.
pub const fn proto_vultr_instance_status_from_domain(
    value: VultrInstanceStatus,
) -> proto::VultrInstanceStatus {
    match value {
        VultrInstanceStatus::Pending => proto::VultrInstanceStatus::VULTR_INSTANCE_STATUS_PENDING,
        VultrInstanceStatus::Active => proto::VultrInstanceStatus::VULTR_INSTANCE_STATUS_ACTIVE,
        VultrInstanceStatus::Stopped => proto::VultrInstanceStatus::VULTR_INSTANCE_STATUS_STOPPED,
        VultrInstanceStatus::Suspended => {
            proto::VultrInstanceStatus::VULTR_INSTANCE_STATUS_SUSPENDED
        }
    }
}

fn empty_string_as_none(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}
