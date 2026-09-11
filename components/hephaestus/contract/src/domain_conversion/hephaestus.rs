// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Conversion between Hephaestus domain DTOs and authoritative domain types.

use buffa::Enumeration as _;

use crate::generated::proto::reallyme::domain::v1 as proto;

use reallyme_hephaestus_domain::{
    DockerContainerName, DockerImageReference, HephaestusServerApp, HephaestusServerService,
    VultrError,
};
use reallyme_hephaestus_domain::{
    HephaestusAgentBootReport, HephaestusAgentBootReportResult, HephaestusAgentReport,
    HephaestusAgentReportLine, HephaestusCadvisorSummary, HephaestusClusterId,
    HephaestusContainerHealthState, HephaestusContainerReport, HephaestusContainerRestartReason,
    HephaestusContainerState, HephaestusDnsDesiredState, HephaestusDnsRecord,
    HephaestusDnsRecordKind, HephaestusDnsRecordName, HephaestusDnsRecordTarget,
    HephaestusDnsRecordVisibility, HephaestusDomainError, HephaestusHostResourceReport,
    HephaestusHostServiceReport, HephaestusHostServiceState, HephaestusHostServiceUnitName,
    HephaestusInfrastructureClassId, HephaestusNodeActualState, HephaestusNodeActualStateStatus,
    HephaestusNodeExporterSummary, HephaestusObservabilityReport, HephaestusObservedServiceSet,
    HephaestusPatchStateReport, HephaestusPrivateIpAddress, HephaestusProviderServerId,
    HephaestusPublicIpAddress, HephaestusRegionDefinition, HephaestusRegionId,
    HephaestusRuntimeVersionReport, HephaestusServerBootTiming, HephaestusServerCatalog,
    HephaestusServerEnvironmentTag, HephaestusServerId, HephaestusServerIdentity,
    HephaestusServerInventoryRow, HephaestusServerNetwork, HephaestusServerRole,
    HephaestusServiceProbeKind, HephaestusServiceProbeReport, HephaestusServiceProbeStatus,
    HephaestusSiteId, HephaestusTailscaleReport, HephaestusUnattendedUpgradesState,
    ProvisioningEventKind, ProvisioningEventSnapshot,
};

/// Converts a protobuf DNS desired-state DTO into a validated domain type.
pub fn dns_desired_state_from_proto(
    value: proto::HephaestusDnsDesiredState,
) -> Result<HephaestusDnsDesiredState, HephaestusDomainError> {
    let records = value
        .records
        .into_iter()
        .map(dns_record_from_proto)
        .collect::<Result<Vec<_>, _>>()?;
    HephaestusDnsDesiredState::new(records)
}

/// Converts a domain DNS desired-state value into a protobuf DTO.
pub fn proto_dns_desired_state_from_domain(
    value: &HephaestusDnsDesiredState,
) -> proto::HephaestusDnsDesiredState {
    proto::HephaestusDnsDesiredState {
        records: value
            .records()
            .iter()
            .map(proto_dns_record_from_domain)
            .collect(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf Region-definition DTO into a validated domain type.
pub fn region_definition_from_proto(
    value: proto::HephaestusRegionDefinition,
) -> Result<HephaestusRegionDefinition, HephaestusDomainError> {
    HephaestusRegionDefinition::new(
        HephaestusRegionId::new(value.region_id)?,
        value
            .site_ids
            .into_iter()
            .map(HephaestusSiteId::new)
            .collect::<Result<Vec<_>, _>>()?,
    )
}

/// Converts a domain Region definition into a protobuf DTO.
pub fn proto_region_definition_from_domain(
    value: &HephaestusRegionDefinition,
) -> proto::HephaestusRegionDefinition {
    proto::HephaestusRegionDefinition {
        region_id: value.region_id().as_str().to_owned(),
        site_ids: value
            .site_ids()
            .iter()
            .map(HephaestusSiteId::as_str)
            .map(str::to_owned)
            .collect(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf agent report DTO into a validated domain type.
pub fn agent_report_from_proto(
    value: proto::HephaestusAgentReport,
) -> Result<HephaestusAgentReport, HephaestusDomainError> {
    let private_ip = empty_string_as_none(value.private_ip)
        .map(HephaestusPrivateIpAddress::new)
        .transpose()?;
    let resources = value
        .resources
        .into_option()
        .ok_or(HephaestusDomainError::InvalidCharacter)
        .and_then(host_resource_report_from_proto)?;

    let report = HephaestusAgentReport::new(
        HephaestusServerId::new(value.server_id)?,
        HephaestusProviderServerId::new(value.provider_server_id)?,
        private_ip,
        value.generated_at_unix_secs,
        resources,
        HephaestusObservedServiceSet::new(
            value
                .containers
                .into_iter()
                .map(container_report_from_proto)
                .collect::<Result<Vec<_>, _>>()?,
            value
                .host_services
                .into_iter()
                .map(host_service_report_from_proto)
                .collect::<Result<Vec<_>, _>>()?,
            value
                .actual_apps
                .into_iter()
                .map(HephaestusServerApp::new)
                .collect::<Result<Vec<_>, _>>()?,
        )?,
    )?;

    report.with_host_health_details(
        value
            .tailscale
            .into_option()
            .map(tailscale_report_from_proto)
            .transpose()?
            .unwrap_or_else(HephaestusTailscaleReport::empty),
        value
            .runtime_versions
            .into_option()
            .map(runtime_version_report_from_proto)
            .transpose()?
            .unwrap_or_else(HephaestusRuntimeVersionReport::empty),
        value
            .patch_state
            .into_option()
            .map(patch_state_report_from_proto)
            .transpose()?
            .unwrap_or_else(HephaestusPatchStateReport::unknown),
        value
            .observability
            .into_option()
            .map(observability_report_from_proto)
            .transpose()?
            .unwrap_or_else(HephaestusObservabilityReport::empty),
        value
            .service_probes
            .into_iter()
            .map(service_probe_report_from_proto)
            .collect::<Result<Vec<_>, _>>()?,
    )
}

/// Converts a domain agent report into a protobuf DTO.
pub fn proto_agent_report_from_domain(
    value: &HephaestusAgentReport,
) -> proto::HephaestusAgentReport {
    proto::HephaestusAgentReport {
        server_id: value.server_id().as_str().to_owned(),
        provider_server_id: value.provider_server_id().as_str().to_owned(),
        private_ip: value
            .private_ip()
            .map(HephaestusPrivateIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        generated_at_unix_secs: value.generated_at_unix_secs(),
        resources: buffa::MessageField::some(proto_host_resource_report_from_domain(
            value.resources(),
        )),
        containers: value
            .containers()
            .iter()
            .map(proto_container_report_from_domain)
            .collect(),
        host_services: value
            .host_services()
            .iter()
            .map(proto_host_service_report_from_domain)
            .collect(),
        actual_apps: value
            .actual_apps()
            .iter()
            .map(HephaestusServerApp::as_str)
            .map(str::to_owned)
            .collect(),
        tailscale: buffa::MessageField::some(proto_tailscale_report_from_domain(value.tailscale())),
        runtime_versions: buffa::MessageField::some(proto_runtime_version_report_from_domain(
            value.runtime_versions(),
        )),
        patch_state: buffa::MessageField::some(proto_patch_state_report_from_domain(
            value.patch_state(),
        )),
        observability: buffa::MessageField::some(proto_observability_report_from_domain(
            value.observability(),
        )),
        service_probes: value
            .service_probes()
            .iter()
            .map(proto_service_probe_report_from_domain)
            .collect(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf boot report DTO into a validated domain type.
pub fn agent_boot_report_from_proto(
    value: proto::HephaestusAgentBootReport,
) -> Result<HephaestusAgentBootReport, HephaestusDomainError> {
    let public_ip = empty_string_as_none(value.public_ip)
        .map(HephaestusPublicIpAddress::new)
        .transpose()?;
    let private_ip = empty_string_as_none(value.private_ip)
        .map(HephaestusPrivateIpAddress::new)
        .transpose()?;

    HephaestusAgentBootReport::new(
        HephaestusServerIdentity::new(
            HephaestusServerId::new(value.server_id)?,
            HephaestusProviderServerId::new(value.provider_server_id)?,
            HephaestusSiteId::new(value.site_id)?,
        ),
        HephaestusServerNetwork::new(public_ip, private_ip),
        HephaestusServerBootTiming::new(value.boot_time_unix_secs, value.generated_at_unix_secs)?,
    )
}

/// Converts a domain boot report into a protobuf DTO.
pub fn proto_agent_boot_report_from_domain(
    value: &HephaestusAgentBootReport,
) -> proto::HephaestusAgentBootReport {
    proto::HephaestusAgentBootReport {
        server_id: value.server_id().as_str().to_owned(),
        provider_server_id: value.provider_server_id().as_str().to_owned(),
        site_id: value.site_id().as_str().to_owned(),
        public_ip: value
            .public_ip()
            .map(HephaestusPublicIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        private_ip: value
            .private_ip()
            .map(HephaestusPrivateIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        boot_time_unix_secs: value.boot_time_unix_secs(),
        generated_at_unix_secs: value.generated_at_unix_secs(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a protobuf actual-state DTO into a validated domain type.
pub fn node_actual_state_from_proto(
    value: proto::HephaestusNodeActualState,
) -> Result<HephaestusNodeActualState, HephaestusDomainError> {
    HephaestusNodeActualState::new(
        HephaestusServerIdentity::new(
            HephaestusServerId::new(value.server_id)?,
            HephaestusProviderServerId::new(value.provider_server_id)?,
            HephaestusSiteId::new(value.site_id)?,
        ),
        HephaestusServerNetwork::new(
            empty_string_as_none(value.public_ip)
                .map(HephaestusPublicIpAddress::new)
                .transpose()?,
            empty_string_as_none(value.private_ip)
                .map(HephaestusPrivateIpAddress::new)
                .transpose()?,
        ),
        HephaestusServerCatalog::new(
            empty_string_as_none(value.cluster_id)
                .map(HephaestusClusterId::new)
                .transpose()?,
            server_role_from_proto_i32(value.role.to_i32())?,
            value
                .apps
                .into_iter()
                .map(HephaestusServerApp::new)
                .collect::<Result<Vec<_>, _>>()?,
            value
                .services
                .into_iter()
                .map(HephaestusServerService::new)
                .collect::<Result<Vec<_>, _>>()?,
        )?,
        HephaestusInfrastructureClassId::new(value.infrastructure_class_id)?,
        value.boot_time_unix_secs,
        value.last_seen_unix_secs,
        node_actual_state_status_from_proto_i32(value.status.to_i32())?,
    )
}

/// Converts a domain actual-state value into a protobuf DTO.
pub fn proto_node_actual_state_from_domain(
    value: &HephaestusNodeActualState,
) -> proto::HephaestusNodeActualState {
    proto::HephaestusNodeActualState {
        server_id: value.server_id().as_str().to_owned(),
        provider_server_id: value.provider_server_id().as_str().to_owned(),
        site_id: value.site_id().as_str().to_owned(),
        public_ip: value
            .public_ip()
            .map(HephaestusPublicIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        private_ip: value
            .private_ip()
            .map(HephaestusPrivateIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        boot_time_unix_secs: value.boot_time_unix_secs(),
        last_seen_unix_secs: value.last_seen_unix_secs(),
        status: buffa::EnumValue::from(
            proto_node_actual_state_status_from_domain(value.status()) as i32
        ),
        cluster_id: value
            .cluster_id()
            .map(HephaestusClusterId::as_str)
            .unwrap_or_default()
            .to_owned(),
        role: buffa::EnumValue::from(proto_server_role_from_domain(value.role()) as i32),
        apps: value
            .apps()
            .iter()
            .map(HephaestusServerApp::as_str)
            .map(str::to_owned)
            .collect(),
        services: value
            .services()
            .iter()
            .map(HephaestusServerService::as_str)
            .map(str::to_owned)
            .collect(),
        infrastructure_class_id: value.infrastructure_class_id().as_str().to_owned(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a domain inventory row into a protobuf DTO.
pub fn proto_server_inventory_row_from_domain(
    value: &HephaestusServerInventoryRow,
) -> proto::HephaestusServerInventoryRow {
    proto::HephaestusServerInventoryRow {
        server_id: value.server_id().as_str().to_owned(),
        provider_server_id: value.provider_server_id().as_str().to_owned(),
        site_id: value.identity().site_id().as_str().to_owned(),
        infrastructure_class_id: value.infrastructure_class_id().as_str().to_owned(),
        cluster_id: value
            .catalog()
            .cluster_id()
            .map(HephaestusClusterId::as_str)
            .unwrap_or_default()
            .to_owned(),
        role: buffa::EnumValue::from(proto_server_role_from_domain(value.catalog().role()) as i32),
        apps: value
            .catalog()
            .apps()
            .iter()
            .map(HephaestusServerApp::as_str)
            .map(str::to_owned)
            .collect(),
        services: value
            .catalog()
            .services()
            .iter()
            .map(HephaestusServerService::as_str)
            .map(str::to_owned)
            .collect(),
        actual_apps: value
            .actual_apps()
            .iter()
            .map(HephaestusServerApp::as_str)
            .map(str::to_owned)
            .collect(),
        assigned_region_id: value
            .assigned_region_id()
            .map(HephaestusRegionId::as_str)
            .unwrap_or_default()
            .to_owned(),
        environment_tag: buffa::EnumValue::from(proto_server_environment_tag_from_domain(
            value.environment_tag(),
        ) as i32),
        planned_private_ip: value
            .planned_private_ip()
            .map(HephaestusPrivateIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        public_ip: value
            .public_ip()
            .map(HephaestusPublicIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        private_ip: value
            .private_ip()
            .map(HephaestusPrivateIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        boot_time_unix_secs: value.boot_time_unix_secs().unwrap_or_default(),
        last_seen_unix_secs: value.last_seen_unix_secs().unwrap_or_default(),
        actual_status: value
            .actual_status()
            .map(proto_node_actual_state_status_from_domain)
            .map(|status| buffa::EnumValue::from(status as i32))
            .unwrap_or_default(),
        latest_provisioning_event: value
            .latest_provisioning_event()
            .map(proto_provisioning_event_snapshot_from_domain)
            .map(buffa::MessageField::some)
            .unwrap_or_default(),
        dns_records: value
            .dns_records()
            .iter()
            .map(proto_dns_record_from_domain)
            .collect(),
        latest_agent_report: value
            .latest_agent_report()
            .map(proto_agent_report_from_domain)
            .map(buffa::MessageField::some)
            .unwrap_or_default(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_server_environment_tag_from_domain(
    value: HephaestusServerEnvironmentTag,
) -> proto::HephaestusServerEnvironmentTag {
    match value {
        HephaestusServerEnvironmentTag::Unspecified => {
            proto::HephaestusServerEnvironmentTag::HEPHAESTUS_SERVER_ENVIRONMENT_TAG_UNSPECIFIED
        }
        HephaestusServerEnvironmentTag::Dev => {
            proto::HephaestusServerEnvironmentTag::HEPHAESTUS_SERVER_ENVIRONMENT_TAG_DEV
        }
        HephaestusServerEnvironmentTag::Prod => {
            proto::HephaestusServerEnvironmentTag::HEPHAESTUS_SERVER_ENVIRONMENT_TAG_PROD
        }
    }
}

/// Converts a domain provisioning event snapshot into a protobuf DTO.
pub fn proto_provisioning_event_snapshot_from_domain(
    value: &ProvisioningEventSnapshot,
) -> proto::ProvisioningEventSnapshot {
    proto::ProvisioningEventSnapshot {
        kind: buffa::EnumValue::from(proto_provisioning_event_kind_from_domain(value.kind()) as i32),
        detail: value.detail().unwrap_or_default().to_owned(),
        __buffa_unknown_fields: Default::default(),
    }
}

/// Converts a domain boot-report result into a protobuf enum.
pub const fn proto_agent_boot_report_result_from_domain(
    value: HephaestusAgentBootReportResult,
) -> proto::HephaestusAgentBootReportResult {
    match value {
        HephaestusAgentBootReportResult::Accepted => {
            proto::HephaestusAgentBootReportResult::HEPHAESTUS_AGENT_BOOT_REPORT_RESULT_ACCEPTED
        }
        HephaestusAgentBootReportResult::RejectedUnknownServer => {
            proto::HephaestusAgentBootReportResult::HEPHAESTUS_AGENT_BOOT_REPORT_RESULT_REJECTED_UNKNOWN_SERVER
        }
        HephaestusAgentBootReportResult::RejectedAuthentication => {
            proto::HephaestusAgentBootReportResult::HEPHAESTUS_AGENT_BOOT_REPORT_RESULT_REJECTED_AUTHENTICATION
        }
    }
}

fn dns_record_from_proto(
    value: proto::HephaestusDnsRecord,
) -> Result<HephaestusDnsRecord, HephaestusDomainError> {
    Ok(HephaestusDnsRecord::new(
        HephaestusDnsRecordName::new(value.name)?,
        dns_record_kind_from_proto_i32(value.kind.to_i32())?,
        HephaestusDnsRecordTarget::new(value.target)?,
        dns_record_visibility_from_proto_i32(value.visibility.to_i32())?,
    ))
}

fn proto_dns_record_from_domain(value: &HephaestusDnsRecord) -> proto::HephaestusDnsRecord {
    proto::HephaestusDnsRecord {
        name: value.name().as_str().to_owned(),
        kind: buffa::EnumValue::from(proto_dns_record_kind_from_domain(value.kind()) as i32),
        target: value.target().as_str().to_owned(),
        visibility: buffa::EnumValue::from(proto_dns_record_visibility_from_domain(
            value.visibility(),
        ) as i32),
        __buffa_unknown_fields: Default::default(),
    }
}

fn host_resource_report_from_proto(
    value: proto::HephaestusHostResourceReport,
) -> Result<HephaestusHostResourceReport, HephaestusDomainError> {
    let cpu_usage_basis_points = u16::try_from(value.cpu_usage_basis_points)
        .map_err(|_error| HephaestusDomainError::InvalidNumber)?;
    HephaestusHostResourceReport::new(
        cpu_usage_basis_points,
        value.memory_total_bytes,
        value.memory_used_bytes,
        value.disk_total_bytes,
        value.disk_used_bytes,
    )
}

fn proto_host_resource_report_from_domain(
    value: &HephaestusHostResourceReport,
) -> proto::HephaestusHostResourceReport {
    proto::HephaestusHostResourceReport {
        cpu_usage_basis_points: u32::from(value.cpu_usage_basis_points()),
        memory_total_bytes: value.memory_total_bytes(),
        memory_used_bytes: value.memory_used_bytes(),
        disk_total_bytes: value.disk_total_bytes(),
        disk_used_bytes: value.disk_used_bytes(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn tailscale_report_from_proto(
    value: proto::HephaestusTailscaleReport,
) -> Result<HephaestusTailscaleReport, HephaestusDomainError> {
    HephaestusTailscaleReport::new(
        empty_string_as_none(value.hostname),
        empty_string_as_none(value.magic_dns_name),
        value.ips,
        value.tags,
        value.services,
    )
}

fn proto_tailscale_report_from_domain(
    value: &HephaestusTailscaleReport,
) -> proto::HephaestusTailscaleReport {
    proto::HephaestusTailscaleReport {
        hostname: value.hostname().unwrap_or_default().to_owned(),
        magic_dns_name: value.magic_dns_name().unwrap_or_default().to_owned(),
        ips: value.ips().to_vec(),
        tags: value.tags().to_vec(),
        services: value.services().to_vec(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn runtime_version_report_from_proto(
    value: proto::HephaestusRuntimeVersionReport,
) -> Result<HephaestusRuntimeVersionReport, HephaestusDomainError> {
    HephaestusRuntimeVersionReport::new(
        empty_string_as_none(value.docker_version),
        empty_string_as_none(value.docker_compose_version),
    )
}

fn proto_runtime_version_report_from_domain(
    value: &HephaestusRuntimeVersionReport,
) -> proto::HephaestusRuntimeVersionReport {
    proto::HephaestusRuntimeVersionReport {
        docker_version: value.docker_version().unwrap_or_default().to_owned(),
        docker_compose_version: value
            .docker_compose_version()
            .unwrap_or_default()
            .to_owned(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn patch_state_report_from_proto(
    value: proto::HephaestusPatchStateReport,
) -> Result<HephaestusPatchStateReport, HephaestusDomainError> {
    Ok(HephaestusPatchStateReport::new(
        value.reboot_required,
        unattended_upgrades_state_from_proto_i32(value.unattended_upgrades_state.to_i32())?,
    ))
}

fn proto_patch_state_report_from_domain(
    value: &HephaestusPatchStateReport,
) -> proto::HephaestusPatchStateReport {
    proto::HephaestusPatchStateReport {
        reboot_required: value.reboot_required(),
        unattended_upgrades_state: buffa::EnumValue::from(
            proto_unattended_upgrades_state_from_domain(value.unattended_upgrades_state()) as i32,
        ),
        __buffa_unknown_fields: Default::default(),
    }
}

fn observability_report_from_proto(
    value: proto::HephaestusObservabilityReport,
) -> Result<HephaestusObservabilityReport, HephaestusDomainError> {
    let cadvisor = value
        .cadvisor
        .into_option()
        .map(cadvisor_summary_from_proto)
        .unwrap_or_else(HephaestusCadvisorSummary::unreachable);
    let node_exporter = value
        .node_exporter
        .into_option()
        .map(node_exporter_summary_from_proto)
        .unwrap_or_else(HephaestusNodeExporterSummary::unreachable);
    Ok(HephaestusObservabilityReport::new(cadvisor, node_exporter))
}

fn proto_observability_report_from_domain(
    value: HephaestusObservabilityReport,
) -> proto::HephaestusObservabilityReport {
    proto::HephaestusObservabilityReport {
        cadvisor: buffa::MessageField::some(proto_cadvisor_summary_from_domain(value.cadvisor())),
        node_exporter: buffa::MessageField::some(proto_node_exporter_summary_from_domain(
            value.node_exporter(),
        )),
        __buffa_unknown_fields: Default::default(),
    }
}

fn cadvisor_summary_from_proto(
    value: proto::HephaestusCadvisorSummary,
) -> HephaestusCadvisorSummary {
    HephaestusCadvisorSummary::new(
        value.reachable,
        value.container_metric_lines,
        value.has_cpu_metrics,
        value.has_memory_metrics,
    )
}

fn proto_cadvisor_summary_from_domain(
    value: HephaestusCadvisorSummary,
) -> proto::HephaestusCadvisorSummary {
    proto::HephaestusCadvisorSummary {
        reachable: value.reachable(),
        container_metric_lines: value.container_metric_lines(),
        has_cpu_metrics: value.has_cpu_metrics(),
        has_memory_metrics: value.has_memory_metrics(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn node_exporter_summary_from_proto(
    value: proto::HephaestusNodeExporterSummary,
) -> HephaestusNodeExporterSummary {
    HephaestusNodeExporterSummary::new(
        value.reachable,
        value.metric_lines,
        value.has_cpu_metrics,
        value.has_memory_metrics,
        value.has_filesystem_metrics,
    )
}

fn proto_node_exporter_summary_from_domain(
    value: HephaestusNodeExporterSummary,
) -> proto::HephaestusNodeExporterSummary {
    proto::HephaestusNodeExporterSummary {
        reachable: value.reachable(),
        metric_lines: value.metric_lines(),
        has_cpu_metrics: value.has_cpu_metrics(),
        has_memory_metrics: value.has_memory_metrics(),
        has_filesystem_metrics: value.has_filesystem_metrics(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn service_probe_report_from_proto(
    value: proto::HephaestusServiceProbeReport,
) -> Result<HephaestusServiceProbeReport, HephaestusDomainError> {
    let http_status_code = if value.http_status_code == 0 {
        None
    } else {
        Some(
            u16::try_from(value.http_status_code)
                .map_err(|_error| HephaestusDomainError::InvalidNumber)?,
        )
    };
    HephaestusServiceProbeReport::new(
        value.probe_name,
        value.service_name,
        service_probe_kind_from_proto_i32(value.kind.to_i32())?,
        value.target,
        service_probe_status_from_proto_i32(value.status.to_i32())?,
        http_status_code,
    )
}

fn proto_service_probe_report_from_domain(
    value: &HephaestusServiceProbeReport,
) -> proto::HephaestusServiceProbeReport {
    proto::HephaestusServiceProbeReport {
        probe_name: value.probe_name().to_owned(),
        service_name: value.service_name().to_owned(),
        kind: buffa::EnumValue::from(proto_service_probe_kind_from_domain(value.kind()) as i32),
        target: value.target().to_owned(),
        status: buffa::EnumValue::from(
            proto_service_probe_status_from_domain(value.status()) as i32
        ),
        http_status_code: value.http_status_code().map(u32::from).unwrap_or_default(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn container_report_from_proto(
    value: proto::HephaestusContainerReport,
) -> Result<HephaestusContainerReport, HephaestusDomainError> {
    HephaestusContainerReport::new(
        DockerContainerName::new(value.container_name).map_err(map_vultr_error)?,
        DockerImageReference::new(value.image).map_err(map_vultr_error)?,
        container_state_from_proto_i32(value.state.to_i32())?,
        container_health_state_from_proto_i32(value.health_state.to_i32())?,
        value
            .recent_log_lines
            .into_iter()
            .map(HephaestusAgentReportLine::new)
            .collect::<Result<Vec<_>, _>>()?,
        value.restart_required,
        container_restart_reason_from_proto_i32(value.restart_reason.to_i32())?,
    )
}

fn proto_container_report_from_domain(
    value: &HephaestusContainerReport,
) -> proto::HephaestusContainerReport {
    proto::HephaestusContainerReport {
        container_name: value.name().as_str().to_owned(),
        image: value.image().as_str().to_owned(),
        state: buffa::EnumValue::from(proto_container_state_from_domain(value.state()) as i32),
        health_state: buffa::EnumValue::from(proto_container_health_state_from_domain(
            value.health(),
        ) as i32),
        recent_log_lines: value
            .recent_logs()
            .iter()
            .map(HephaestusAgentReportLine::as_str)
            .map(str::to_owned)
            .collect(),
        restart_required: value.restart_required(),
        restart_reason: buffa::EnumValue::from(proto_container_restart_reason_from_domain(
            value.restart_reason(),
        ) as i32),
        __buffa_unknown_fields: Default::default(),
    }
}

fn host_service_report_from_proto(
    value: proto::HephaestusHostServiceReport,
) -> Result<HephaestusHostServiceReport, HephaestusDomainError> {
    HephaestusHostServiceReport::new(
        HephaestusHostServiceUnitName::new(value.unit_name)?,
        host_service_state_from_proto_i32(value.state.to_i32())?,
        value
            .recent_log_lines
            .into_iter()
            .map(HephaestusAgentReportLine::new)
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn proto_host_service_report_from_domain(
    value: &HephaestusHostServiceReport,
) -> proto::HephaestusHostServiceReport {
    proto::HephaestusHostServiceReport {
        unit_name: value.unit_name().as_str().to_owned(),
        state: buffa::EnumValue::from(proto_host_service_state_from_domain(value.state()) as i32),
        recent_log_lines: value
            .recent_logs()
            .iter()
            .map(HephaestusAgentReportLine::as_str)
            .map(str::to_owned)
            .collect(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn dns_record_kind_from_proto_i32(
    value: i32,
) -> Result<HephaestusDnsRecordKind, HephaestusDomainError> {
    match proto::HephaestusDnsRecordKind::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusDnsRecordKind::HEPHAESTUS_DNS_RECORD_KIND_A => {
            Ok(HephaestusDnsRecordKind::A)
        }
        proto::HephaestusDnsRecordKind::HEPHAESTUS_DNS_RECORD_KIND_AAAA => {
            Ok(HephaestusDnsRecordKind::Aaaa)
        }
        proto::HephaestusDnsRecordKind::HEPHAESTUS_DNS_RECORD_KIND_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_dns_record_kind_from_domain(
    value: HephaestusDnsRecordKind,
) -> proto::HephaestusDnsRecordKind {
    match value {
        HephaestusDnsRecordKind::A => proto::HephaestusDnsRecordKind::HEPHAESTUS_DNS_RECORD_KIND_A,
        HephaestusDnsRecordKind::Aaaa => {
            proto::HephaestusDnsRecordKind::HEPHAESTUS_DNS_RECORD_KIND_AAAA
        }
    }
}

fn dns_record_visibility_from_proto_i32(
    value: i32,
) -> Result<HephaestusDnsRecordVisibility, HephaestusDomainError> {
    match proto::HephaestusDnsRecordVisibility::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusDnsRecordVisibility::HEPHAESTUS_DNS_RECORD_VISIBILITY_PUBLIC => {
            Ok(HephaestusDnsRecordVisibility::Public)
        }
        proto::HephaestusDnsRecordVisibility::HEPHAESTUS_DNS_RECORD_VISIBILITY_INTERNAL => {
            Ok(HephaestusDnsRecordVisibility::Internal)
        }
        proto::HephaestusDnsRecordVisibility::HEPHAESTUS_DNS_RECORD_VISIBILITY_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_dns_record_visibility_from_domain(
    value: HephaestusDnsRecordVisibility,
) -> proto::HephaestusDnsRecordVisibility {
    match value {
        HephaestusDnsRecordVisibility::Public => {
            proto::HephaestusDnsRecordVisibility::HEPHAESTUS_DNS_RECORD_VISIBILITY_PUBLIC
        }
        HephaestusDnsRecordVisibility::Internal => {
            proto::HephaestusDnsRecordVisibility::HEPHAESTUS_DNS_RECORD_VISIBILITY_INTERNAL
        }
    }
}

fn container_state_from_proto_i32(
    value: i32,
) -> Result<HephaestusContainerState, HephaestusDomainError> {
    match proto::HephaestusContainerState::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_CREATED => {
            Ok(HephaestusContainerState::Created)
        }
        proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_RUNNING => {
            Ok(HephaestusContainerState::Running)
        }
        proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_RESTARTING => {
            Ok(HephaestusContainerState::Restarting)
        }
        proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_REMOVING => {
            Ok(HephaestusContainerState::Removing)
        }
        proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_PAUSED => {
            Ok(HephaestusContainerState::Paused)
        }
        proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_EXITED => {
            Ok(HephaestusContainerState::Exited)
        }
        proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_DEAD => {
            Ok(HephaestusContainerState::Dead)
        }
        proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_UNKNOWN => {
            Ok(HephaestusContainerState::Unknown)
        }
        proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_container_state_from_domain(
    value: HephaestusContainerState,
) -> proto::HephaestusContainerState {
    match value {
        HephaestusContainerState::Created => {
            proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_CREATED
        }
        HephaestusContainerState::Running => {
            proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_RUNNING
        }
        HephaestusContainerState::Restarting => {
            proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_RESTARTING
        }
        HephaestusContainerState::Removing => {
            proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_REMOVING
        }
        HephaestusContainerState::Paused => {
            proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_PAUSED
        }
        HephaestusContainerState::Exited => {
            proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_EXITED
        }
        HephaestusContainerState::Dead => {
            proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_DEAD
        }
        HephaestusContainerState::Unknown => {
            proto::HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_UNKNOWN
        }
    }
}

fn container_health_state_from_proto_i32(
    value: i32,
) -> Result<HephaestusContainerHealthState, HephaestusDomainError> {
    match proto::HephaestusContainerHealthState::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_NONE => {
            Ok(HephaestusContainerHealthState::None)
        }
        proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_STARTING => {
            Ok(HephaestusContainerHealthState::Starting)
        }
        proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_HEALTHY => {
            Ok(HephaestusContainerHealthState::Healthy)
        }
        proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_UNHEALTHY => {
            Ok(HephaestusContainerHealthState::Unhealthy)
        }
        proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_UNKNOWN => {
            Ok(HephaestusContainerHealthState::Unknown)
        }
        proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_container_health_state_from_domain(
    value: HephaestusContainerHealthState,
) -> proto::HephaestusContainerHealthState {
    match value {
        HephaestusContainerHealthState::None => {
            proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_NONE
        }
        HephaestusContainerHealthState::Starting => {
            proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_STARTING
        }
        HephaestusContainerHealthState::Healthy => {
            proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_HEALTHY
        }
        HephaestusContainerHealthState::Unhealthy => {
            proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_UNHEALTHY
        }
        HephaestusContainerHealthState::Unknown => {
            proto::HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_UNKNOWN
        }
    }
}

fn container_restart_reason_from_proto_i32(
    value: i32,
) -> Result<HephaestusContainerRestartReason, HephaestusDomainError> {
    match proto::HephaestusContainerRestartReason::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_NONE => {
            Ok(HephaestusContainerRestartReason::None)
        }
        proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_UNHEALTHY => {
            Ok(HephaestusContainerRestartReason::Unhealthy)
        }
        proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_RESTARTING => {
            Ok(HephaestusContainerRestartReason::Restarting)
        }
        proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_EXITED => {
            Ok(HephaestusContainerRestartReason::Exited)
        }
        proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_DEAD => {
            Ok(HephaestusContainerRestartReason::Dead)
        }
        proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_UNKNOWN => {
            Ok(HephaestusContainerRestartReason::Unknown)
        }
        proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_container_restart_reason_from_domain(
    value: HephaestusContainerRestartReason,
) -> proto::HephaestusContainerRestartReason {
    match value {
        HephaestusContainerRestartReason::None => {
            proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_NONE
        }
        HephaestusContainerRestartReason::Unhealthy => {
            proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_UNHEALTHY
        }
        HephaestusContainerRestartReason::Restarting => {
            proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_RESTARTING
        }
        HephaestusContainerRestartReason::Exited => {
            proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_EXITED
        }
        HephaestusContainerRestartReason::Dead => {
            proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_DEAD
        }
        HephaestusContainerRestartReason::Unknown => {
            proto::HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_UNKNOWN
        }
    }
}

fn host_service_state_from_proto_i32(
    value: i32,
) -> Result<HephaestusHostServiceState, HephaestusDomainError> {
    match proto::HephaestusHostServiceState::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_ACTIVE => {
            Ok(HephaestusHostServiceState::Active)
        }
        proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_RELOADING => {
            Ok(HephaestusHostServiceState::Reloading)
        }
        proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_INACTIVE => {
            Ok(HephaestusHostServiceState::Inactive)
        }
        proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_FAILED => {
            Ok(HephaestusHostServiceState::Failed)
        }
        proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_ACTIVATING => {
            Ok(HephaestusHostServiceState::Activating)
        }
        proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_DEACTIVATING => {
            Ok(HephaestusHostServiceState::Deactivating)
        }
        proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_UNKNOWN => {
            Ok(HephaestusHostServiceState::Unknown)
        }
        proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_host_service_state_from_domain(
    value: HephaestusHostServiceState,
) -> proto::HephaestusHostServiceState {
    match value {
        HephaestusHostServiceState::Active => {
            proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_ACTIVE
        }
        HephaestusHostServiceState::Reloading => {
            proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_RELOADING
        }
        HephaestusHostServiceState::Inactive => {
            proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_INACTIVE
        }
        HephaestusHostServiceState::Failed => {
            proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_FAILED
        }
        HephaestusHostServiceState::Activating => {
            proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_ACTIVATING
        }
        HephaestusHostServiceState::Deactivating => {
            proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_DEACTIVATING
        }
        HephaestusHostServiceState::Unknown => {
            proto::HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_UNKNOWN
        }
    }
}

fn unattended_upgrades_state_from_proto_i32(
    value: i32,
) -> Result<HephaestusUnattendedUpgradesState, HephaestusDomainError> {
    match proto::HephaestusUnattendedUpgradesState::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_ACTIVE => {
            Ok(HephaestusUnattendedUpgradesState::Active)
        }
        proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_INACTIVE => {
            Ok(HephaestusUnattendedUpgradesState::Inactive)
        }
        proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_FAILED => {
            Ok(HephaestusUnattendedUpgradesState::Failed)
        }
        proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_NOT_INSTALLED => {
            Ok(HephaestusUnattendedUpgradesState::NotInstalled)
        }
        proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_UNKNOWN => {
            Ok(HephaestusUnattendedUpgradesState::Unknown)
        }
        proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_unattended_upgrades_state_from_domain(
    value: HephaestusUnattendedUpgradesState,
) -> proto::HephaestusUnattendedUpgradesState {
    match value {
        HephaestusUnattendedUpgradesState::Active => {
            proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_ACTIVE
        }
        HephaestusUnattendedUpgradesState::Inactive => {
            proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_INACTIVE
        }
        HephaestusUnattendedUpgradesState::Failed => {
            proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_FAILED
        }
        HephaestusUnattendedUpgradesState::NotInstalled => {
            proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_NOT_INSTALLED
        }
        HephaestusUnattendedUpgradesState::Unknown => {
            proto::HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_UNKNOWN
        }
    }
}

fn service_probe_kind_from_proto_i32(
    value: i32,
) -> Result<HephaestusServiceProbeKind, HephaestusDomainError> {
    match proto::HephaestusServiceProbeKind::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusServiceProbeKind::HEPHAESTUS_SERVICE_PROBE_KIND_HTTP => {
            Ok(HephaestusServiceProbeKind::Http)
        }
        proto::HephaestusServiceProbeKind::HEPHAESTUS_SERVICE_PROBE_KIND_TCP => {
            Ok(HephaestusServiceProbeKind::Tcp)
        }
        proto::HephaestusServiceProbeKind::HEPHAESTUS_SERVICE_PROBE_KIND_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_service_probe_kind_from_domain(
    value: HephaestusServiceProbeKind,
) -> proto::HephaestusServiceProbeKind {
    match value {
        HephaestusServiceProbeKind::Http => {
            proto::HephaestusServiceProbeKind::HEPHAESTUS_SERVICE_PROBE_KIND_HTTP
        }
        HephaestusServiceProbeKind::Tcp => {
            proto::HephaestusServiceProbeKind::HEPHAESTUS_SERVICE_PROBE_KIND_TCP
        }
    }
}

fn service_probe_status_from_proto_i32(
    value: i32,
) -> Result<HephaestusServiceProbeStatus, HephaestusDomainError> {
    match proto::HephaestusServiceProbeStatus::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusServiceProbeStatus::HEPHAESTUS_SERVICE_PROBE_STATUS_OK => {
            Ok(HephaestusServiceProbeStatus::Ok)
        }
        proto::HephaestusServiceProbeStatus::HEPHAESTUS_SERVICE_PROBE_STATUS_FAILED => {
            Ok(HephaestusServiceProbeStatus::Failed)
        }
        proto::HephaestusServiceProbeStatus::HEPHAESTUS_SERVICE_PROBE_STATUS_SKIPPED => {
            Ok(HephaestusServiceProbeStatus::Skipped)
        }
        proto::HephaestusServiceProbeStatus::HEPHAESTUS_SERVICE_PROBE_STATUS_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_service_probe_status_from_domain(
    value: HephaestusServiceProbeStatus,
) -> proto::HephaestusServiceProbeStatus {
    match value {
        HephaestusServiceProbeStatus::Ok => {
            proto::HephaestusServiceProbeStatus::HEPHAESTUS_SERVICE_PROBE_STATUS_OK
        }
        HephaestusServiceProbeStatus::Failed => {
            proto::HephaestusServiceProbeStatus::HEPHAESTUS_SERVICE_PROBE_STATUS_FAILED
        }
        HephaestusServiceProbeStatus::Skipped => {
            proto::HephaestusServiceProbeStatus::HEPHAESTUS_SERVICE_PROBE_STATUS_SKIPPED
        }
    }
}

fn node_actual_state_status_from_proto_i32(
    value: i32,
) -> Result<HephaestusNodeActualStateStatus, HephaestusDomainError> {
    match proto::HephaestusNodeActualStateStatus::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusNodeActualStateStatus::HEPHAESTUS_NODE_ACTUAL_STATE_STATUS_READY => {
            Ok(HephaestusNodeActualStateStatus::Ready)
        }
        proto::HephaestusNodeActualStateStatus::HEPHAESTUS_NODE_ACTUAL_STATE_STATUS_STALE => {
            Ok(HephaestusNodeActualStateStatus::Stale)
        }
        proto::HephaestusNodeActualStateStatus::HEPHAESTUS_NODE_ACTUAL_STATE_STATUS_OFFLINE => {
            Ok(HephaestusNodeActualStateStatus::Offline)
        }
        proto::HephaestusNodeActualStateStatus::HEPHAESTUS_NODE_ACTUAL_STATE_STATUS_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_node_actual_state_status_from_domain(
    value: HephaestusNodeActualStateStatus,
) -> proto::HephaestusNodeActualStateStatus {
    match value {
        HephaestusNodeActualStateStatus::Ready => {
            proto::HephaestusNodeActualStateStatus::HEPHAESTUS_NODE_ACTUAL_STATE_STATUS_READY
        }
        HephaestusNodeActualStateStatus::Stale => {
            proto::HephaestusNodeActualStateStatus::HEPHAESTUS_NODE_ACTUAL_STATE_STATUS_STALE
        }
        HephaestusNodeActualStateStatus::Offline => {
            proto::HephaestusNodeActualStateStatus::HEPHAESTUS_NODE_ACTUAL_STATE_STATUS_OFFLINE
        }
    }
}

fn server_role_from_proto_i32(value: i32) -> Result<HephaestusServerRole, HephaestusDomainError> {
    match proto::HephaestusServerRole::from_i32(value)
        .ok_or(HephaestusDomainError::InvalidNumber)?
    {
        proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_API => Ok(HephaestusServerRole::Api),
        proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_WORKER => {
            Ok(HephaestusServerRole::Worker)
        }
        proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_DB => Ok(HephaestusServerRole::Db),
        proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_SEARCH => {
            Ok(HephaestusServerRole::Search)
        }
        proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_EDGE => Ok(HephaestusServerRole::Edge),
        proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_UNKNOWN => {
            Ok(HephaestusServerRole::Unknown)
        }
        proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_UNSPECIFIED => {
            Err(HephaestusDomainError::InvalidNumber)
        }
    }
}

fn proto_server_role_from_domain(value: HephaestusServerRole) -> proto::HephaestusServerRole {
    match value {
        HephaestusServerRole::Api => proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_API,
        HephaestusServerRole::Worker => proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_WORKER,
        HephaestusServerRole::Db => proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_DB,
        HephaestusServerRole::Search => proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_SEARCH,
        HephaestusServerRole::Edge => proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_EDGE,
        HephaestusServerRole::Unknown => {
            proto::HephaestusServerRole::HEPHAESTUS_SERVER_ROLE_UNKNOWN
        }
    }
}

fn proto_provisioning_event_kind_from_domain(
    value: ProvisioningEventKind,
) -> proto::ProvisioningEventKind {
    match value {
        ProvisioningEventKind::Requested => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_REQUESTED
        }
        ProvisioningEventKind::InstanceCreateStarted => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_INSTANCE_CREATE_STARTED
        }
        ProvisioningEventKind::InstanceCreateSucceeded => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_INSTANCE_CREATE_SUCCEEDED
        }
        ProvisioningEventKind::InstanceCreateFailed => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_INSTANCE_CREATE_FAILED
        }
        ProvisioningEventKind::DnsReconcileStarted => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_DNS_RECONCILE_STARTED
        }
        ProvisioningEventKind::DnsReconcileSucceeded => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_DNS_RECONCILE_SUCCEEDED
        }
        ProvisioningEventKind::DnsReconcileFailed => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_DNS_RECONCILE_FAILED
        }
        ProvisioningEventKind::HealthCheckStarted => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_HEALTH_CHECK_STARTED
        }
        ProvisioningEventKind::HealthCheckSucceeded => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_HEALTH_CHECK_SUCCEEDED
        }
        ProvisioningEventKind::HealthCheckFailed => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_HEALTH_CHECK_FAILED
        }
        ProvisioningEventKind::BootReported => {
            proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_BOOT_REPORTED
        }
        ProvisioningEventKind::Ready => proto::ProvisioningEventKind::PROVISIONING_EVENT_KIND_READY,
    }
}

fn empty_string_as_none(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn map_vultr_error(error: VultrError) -> HephaestusDomainError {
    match error {
        VultrError::Empty => HephaestusDomainError::Empty,
        VultrError::TooLong => HephaestusDomainError::TooLong,
        VultrError::InvalidCharacter => HephaestusDomainError::InvalidCharacter,
        VultrError::InvalidNumber
        | VultrError::UnknownInstanceStatus
        | VultrError::UnknownPlanType => HephaestusDomainError::InvalidNumber,
    }
}
