// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(missing_docs)]

use std::collections::BTreeMap;

use reallyme_hephaestus_domain::{
    HephaestusAgentReport, HephaestusNodeActualState, HephaestusServerId,
    HephaestusServerInventoryRow,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusTimestamp {
    pub seconds: i64,
    pub nanos: i32,
}

impl HephaestusTimestamp {
    pub const fn new(seconds: i64, nanos: i32) -> Self {
        Self { seconds, nanos }
    }

    pub fn from_unix_secs(seconds: u64) -> Self {
        const I64_MAX_AS_U64: u64 = 9_223_372_036_854_775_807;
        if seconds > I64_MAX_AS_U64 {
            return Self {
                seconds: i64::MAX,
                nanos: 0,
            };
        }
        let Ok(seconds) = i64::try_from(seconds) else {
            return Self {
                seconds: i64::MAX,
                nanos: 0,
            };
        };
        Self { seconds, nanos: 0 }
    }

    pub fn unix_secs(self) -> u64 {
        let Ok(seconds) = u64::try_from(self.seconds) else {
            return 0;
        };
        seconds
    }
}

/// Host-neutral status request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusStatusRequest;

/// Host-neutral status response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HephaestusStatusResponse {
    body: &'static str,
}

impl HephaestusStatusResponse {
    /// Constructs a status response.
    pub const fn new(body: &'static str) -> Self {
        Self { body }
    }

    /// Returns the response body.
    pub const fn body(self) -> &'static str {
        self.body
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetDesiredTopologyRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetDesiredTopologyResponse {
    pub snapshot: DesiredTopologySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateDesiredTopologyRequest {
    pub base_git_commit: Option<String>,
    pub documents: Vec<DesiredTopologyDocument>,
    pub changed_by: WorkflowActor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateDesiredTopologyResponse {
    pub snapshot: DesiredTopologySnapshot,
    pub git_diff: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListDeployableServicesRequest {
    pub category: Option<DeployableServiceCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListDeployableServicesResponse {
    pub services: Vec<DeployableServiceSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetDeployableServiceRequest {
    pub service_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetDeployableServiceResponse {
    pub service: DeployableService,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListDeployableServiceImagesRequest {
    pub service_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListDeployableServiceImagesResponse {
    pub service_id: String,
    pub images: Vec<DeployableServiceImage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetDeployableServiceImageRequest {
    pub service_id: String,
    pub image_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetDeployableServiceImageResponse {
    pub service_id: String,
    pub image: DeployableServiceImage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployableServiceSummary {
    pub service_id: String,
    pub display_name: String,
    pub category: DeployableServiceCategory,
    pub workload_kind: DeployableWorkloadKind,
    pub short_description: String,
    pub supports_ipv6_only: bool,
    pub requires_ipv4: bool,
    pub default_image_ref: String,
    pub default_ports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployableService {
    pub summary: DeployableServiceSummary,
    pub description: String,
    pub images: Vec<DeployableServiceImage>,
    pub required_secrets: Vec<DeployableServiceSecret>,
    pub supported_actions: Vec<DeployableServiceAction>,
    pub health_signals: Vec<String>,
    pub deployment_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployableServiceImage {
    pub image_id: String,
    pub registry: String,
    pub repository: String,
    pub tag: String,
    pub digest: Option<String>,
    pub default_image: bool,
    pub image_ref: String,
    pub published_at_unix_seconds: Option<u64>,
    pub source_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployableServiceSecret {
    pub secret_ref: String,
    pub display_name: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployableServiceCategory {
    Messaging,
    Search,
    Database,
    Observability,
    Edge,
    ControlPlane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployableWorkloadKind {
    Docker,
    FoundationDb,
    ControlPlane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployableServiceAction {
    Deploy,
    UpgradeImage,
    RestartContainer,
    RestartService,
    RebootHost,
    ResizeNode,
    DeleteNode,
    ReconfigureCluster,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesiredTopologySnapshot {
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub documents: Vec<DesiredTopologyDocument>,
    pub environments: Vec<InfrastructureEnvironment>,
    pub runbook_catalog: Option<RunbookCatalog>,
    pub docker_image_catalog: Option<DockerImageCatalog>,
    pub provider_catalogs: Vec<ProviderCatalog>,
    pub health_signal_catalog: Option<HealthSignalCatalog>,
    pub rbac_policy: Option<RbacPolicyCatalog>,
    pub docker_service_definitions: Vec<DockerServiceDefinition>,
    pub action_request_examples: Vec<InfrastructureActionRequest>,
    pub json_schemas: Vec<InfrastructureJsonSchemaDocument>,
    pub operational_documents: Vec<InfrastructureOperationalDocument>,
    pub service_mesh: Option<ServiceMeshProjection>,
    pub internal_dns: Option<InternalDnsProjection>,
    pub observability: Option<ObservabilityProjection>,
    pub node_topology: Option<NodeTopologyProjection>,
    pub dashboard_control_surface: Option<DashboardControlSurfaceProjection>,
    pub control_plane_surfaces: Option<ControlPlaneSurfaceCatalog>,
    pub git_diff: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesiredTopologyDocument {
    pub path: String,
    pub body: DesiredTopologyDocumentBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "document", rename_all = "snake_case")]
pub enum DesiredTopologyDocumentBody {
    Global(GlobalTopologyDocument),
    Environment(EnvironmentTopologyDocument),
    Region(RegionTopologyDocument),
    Services(ServicesTopologyDocument),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalTopologyDocument {
    pub tenants: Vec<TenantDefinition>,
    pub failover_policies: Vec<FailoverPolicy>,
    pub rollout_policies: Vec<RolloutPolicy>,
    pub service_dependencies: Vec<ServiceDependency>,
    pub networking: Option<GlobalNetworkingTopology>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentTopologyDocument {
    pub environment_id: String,
    pub deployment_tier: String,
    pub region_ids: Vec<String>,
    pub topology_document_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionTopologyDocument {
    pub region: RegionDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServicesTopologyDocument {
    pub service_groups: Vec<ServiceGroupDefinition>,
    pub services: Vec<ServiceDefinition>,
    pub tenant_region_rules: Vec<TenantRegionRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionDefinition {
    pub region_id: String,
    pub jurisdiction: String,
    pub transport_policy: Option<NetworkTransportPolicy>,
    pub internal_dns: Option<InternalDnsPolicy>,
    pub service_subnet_plan: Vec<ServiceSubnetPlanEntry>,
    pub sites: Vec<SiteDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteDefinition {
    pub site_id: String,
    pub jurisdiction: String,
    pub datacenter_id: String,
    pub provider: InfrastructureProvider,
    pub provider_location_id: String,
    pub planned_vpc_cidr: Option<String>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalNetworkingTopology {
    pub same_site_service_transport: String,
    pub cross_site_service_transport: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkTransportPolicy {
    pub same_site_service_transport: String,
    pub cross_site_service_transport: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalDnsPolicy {
    pub zone: String,
    pub service_naming_policy: String,
    pub node_naming_policy: String,
    pub service_record_strategy: String,
    pub site_token_source: String,
    pub services: BTreeMap<String, String>,
    pub tailscale_suffix_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalDnsProjection {
    pub schema_version: u32,
    pub topology_contract: InternalDnsTopologyContract,
    pub naming_policy: InternalDnsNamingPolicy,
    pub provider_site_token_policy: ProviderSiteTokenPolicy,
    pub generated_output_contract: InternalDnsGeneratedOutputContract,
    pub service_labels: BTreeMap<String, String>,
    pub node_name_templates: BTreeMap<String, String>,
    pub node_dns_templates: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalDnsTopologyContract {
    pub source_file: String,
    pub section: String,
    pub required_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalDnsNamingPolicy {
    pub service_record: String,
    pub node_record: String,
    pub round_robin_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSiteTokenPolicy {
    pub site_token_source: String,
    pub site_token_format: String,
    pub normalization: String,
    pub intended_uses: Vec<String>,
    pub operator_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternalDnsGeneratedOutputContract {
    pub output_name: String,
    pub sections: Vec<String>,
    pub policy_fields: Vec<String>,
    pub node_record_fields: Vec<String>,
    pub service_record_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityProjection {
    pub schema_version: u32,
    pub host_agents: ObservabilityHostAgents,
    pub scrape_target_generator: PrometheusScrapeTargetGenerator,
    pub enforcement_layers: ObservabilityEnforcementLayers,
    pub source_catalogs: ObservabilitySourceCatalogs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityHostAgents {
    pub node_exporter: ObservabilityAgentContract,
    pub cadvisor: ObservabilityAgentContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityAgentContract {
    pub install_scope: String,
    pub deployment_boundary: String,
    pub private_only: bool,
    pub default_port: u32,
    pub source_action_id: String,
    pub required_fields: Vec<String>,
    pub field_contract: BTreeMap<String, ObservabilityFieldContract>,
    pub health_signal_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityFieldContract {
    pub value_type: String,
    pub format: Option<String>,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub values: Vec<String>,
    pub healthy_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrometheusScrapeTargetGenerator {
    pub executable: String,
    pub output_format: String,
    pub jobs: Vec<PrometheusScrapeJob>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrometheusScrapeJob {
    pub job_name: String,
    pub metrics_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityEnforcementLayers {
    pub nftables: ObservabilityEnforcementLayer,
    pub tailscale_grants: ObservabilityEnforcementLayer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityEnforcementLayer {
    pub source_of_truth: String,
    pub policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilitySourceCatalogs {
    pub health_signal_catalog_id: Option<String>,
    pub topology_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSubnetPlanEntry {
    pub service_name: String,
    pub cidr_suffix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantDefinition {
    pub tenant_id: String,
    pub jurisdiction_ids: Vec<String>,
    pub home_region_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceGroupDefinition {
    pub group_id: String,
    pub kind: ServiceKind,
    pub member_service_ids: Vec<String>,
    pub region_policy: RegionPolicy,
    pub rollout_policy: RolloutPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceDefinition {
    pub service_id: String,
    pub kind: ServiceKind,
    pub group_id: Option<String>,
    pub depends_on_service_ids: Vec<String>,
    pub region_policy: RegionPolicy,
    pub replication_policy: ReplicationPolicy,
    pub rollout_policy: RolloutPolicy,
    pub spec: ServiceSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "spec", rename_all = "snake_case")]
pub enum ServiceSpec {
    Foundationdb(FoundationDbServiceSpec),
    Nats(NatsSuperclusterServiceSpec),
    Typesense(TypesenseServiceSpec),
    Application(ApplicationServiceSpec),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceKind {
    Foundationdb,
    Nats,
    Typesense,
    NuxtSsr,
    RustApi,
    Graphql,
    AppRuntime,
    LoadBalancer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionPolicy {
    pub preferred_data_regions: Vec<ProvisioningDataRegion>,
    pub preferred_sites: Vec<ProvisioningSite>,
    pub minimum_instance_count: u32,
    pub tenant_affinity: TenantAffinityKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantAffinityKind {
    None,
    Preferred,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationPolicy {
    pub mode: ReplicationMode,
    pub replica_count: u32,
    pub replica_data_regions: Vec<ProvisioningDataRegion>,
    pub witness_sites: Vec<ProvisioningSite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationMode {
    SingleRegion,
    MultiRegionActivePassive,
    MultiRegionActiveActive,
    ServiceDefined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutPolicy {
    pub policy_id: String,
    pub phases: Vec<RolloutPhase>,
    pub require_manual_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutPhase {
    pub order: u32,
    pub service_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceDependency {
    pub upstream_service_id: String,
    pub downstream_service_id: String,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    StartsBefore,
    HealthyBefore,
    FailoverCoordinated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantRegionRule {
    pub tenant_id: String,
    pub region_targets: Vec<TenantRegionTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantRegionTarget {
    pub service_id: String,
    pub data_region: Option<ProvisioningDataRegion>,
    pub site: Option<ProvisioningSite>,
    pub failover_policy_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailoverPolicy {
    pub policy_id: String,
    pub mode: FailoverMode,
    pub primary_data_regions: Vec<ProvisioningDataRegion>,
    pub secondary_data_regions: Vec<ProvisioningDataRegion>,
    pub service_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailoverMode {
    Manual,
    Automated,
    Assisted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbServiceSpec {
    pub cluster_name: String,
    pub version: String,
    pub storage_engine: FoundationDbStorageEngine,
    pub redundancy_mode: FoundationDbRedundancyMode,
    pub usable_regions: u32,
    pub disk_encryption_mode: FoundationDbDiskEncryptionMode,
    pub primary_site: ProvisioningSite,
    pub secondary_site: ProvisioningSite,
    pub witness_site: ProvisioningSite,
    pub sites: Vec<FoundationDbSiteBinding>,
    pub tenant_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbSiteBinding {
    pub site: ProvisioningSite,
    pub role: FoundationDbSiteRole,
    pub node_count: u32,
    pub instance_plan: ProvisioningInstancePlan,
    pub provider: InfrastructureProvider,
    pub provider_location_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundationDbStorageEngine {
    Ssd2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundationDbRedundancyMode {
    Single,
    Double,
    Triple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundationDbDiskEncryptionMode {
    Provider,
    LuksManualUnlock,
    LuksKeyfile,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundationDbSiteRole {
    Primary,
    Secondary,
    Coordinator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningFoundationDbRole {
    Primary,
    Secondary,
    Witness,
    Coordinator,
}

impl ProvisioningFoundationDbRole {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Witness => "witness",
            Self::Coordinator => "coordinator",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanFoundationDbDeploymentRequest {
    pub operation: FoundationDbLifecycleOperation,
    pub intent: FoundationDbClusterIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundationDbLifecycleOperation {
    CreateCluster,
    AddNode,
    ReconfigureCluster,
    Status,
    Destroy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanFoundationDbDeploymentResponse {
    pub plan: Option<FoundationDbDeploymentPlan>,
    pub errors: Vec<FoundationDbValidationError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbClusterIntent {
    pub environment: ProvisioningEnvironment,
    pub data_region: ProvisioningDataRegion,
    pub cluster_id: String,
    pub cluster_description: String,
    pub version: String,
    pub storage_engine: FoundationDbStorageEngine,
    pub redundancy_mode: FoundationDbRedundancyMode,
    pub usable_regions: u32,
    pub disk_encryption_mode: FoundationDbDiskEncryptionMode,
    pub listen_port: u32,
    pub data_mount_point: String,
    pub data_dir: String,
    pub log_dir: String,
    pub cluster_file_path: String,
    pub config_path: String,
    pub primary_site: ProvisioningSite,
    pub secondary_site: ProvisioningSite,
    pub witness_site: ProvisioningSite,
    pub sites: Vec<FoundationDbSiteIntent>,
    pub tenant_ids: Vec<String>,
    pub backup: Option<FoundationDbBackupIntent>,
    pub restore: Option<FoundationDbRestoreIntent>,
    pub private_peer_cidr_count: u32,
    pub admin_peer_cidr_count: u32,
    pub existing_coordinators: Vec<FoundationDbCoordinator>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbSiteIntent {
    pub site: ProvisioningSite,
    pub role: ProvisioningFoundationDbRole,
    pub provider: InfrastructureProvider,
    pub instance_plan: ProvisioningInstancePlan,
    pub node_count: u32,
    pub provider_location_id: String,
    pub hostname_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbBackupIntent {
    pub enabled: bool,
    pub tag: String,
    pub destination_name: String,
    pub bucket_config_ref: String,
    pub access_key_secret_ref: ProvisioningSecretRef,
    pub secret_key_secret_ref: ProvisioningSecretRef,
    pub snapshot_interval_seconds: u32,
    pub initial_snapshot_interval_seconds: u32,
    pub server_side_encryption_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbRestoreIntent {
    pub mode: FoundationDbRestoreMode,
    pub selector: Option<FoundationDbPlannerRestoreSelector>,
    pub source_config_ref: String,
    pub target_empty_confirmed: bool,
    pub force_live_restore: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FoundationDbRestoreMode {
    Disabled,
    DryRun,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FoundationDbPlannerRestoreSelector {
    Timestamp(String),
    Version(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbDeploymentPlan {
    pub runtime_config: FoundationDbRuntimeConfig,
    pub nodes: Vec<FoundationDbNodePlan>,
    pub private_tcp_ports: Vec<u32>,
    pub coordinator_plan: Vec<FoundationDbCoordinator>,
    pub backup_plan: Option<FoundationDbBackupPlan>,
    pub restore_plan: Option<FoundationDbRestorePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbRuntimeConfig {
    pub operation: FoundationDbLifecycleOperation,
    pub environment: ProvisioningEnvironment,
    pub data_region: ProvisioningDataRegion,
    pub cluster_id: String,
    pub cluster_description: String,
    pub version: String,
    pub storage_engine: FoundationDbStorageEngine,
    pub redundancy_mode: FoundationDbRedundancyMode,
    pub usable_regions: u32,
    pub disk_encryption_mode: FoundationDbDiskEncryptionMode,
    pub listen_port: u32,
    pub data_mount_point: String,
    pub data_dir: String,
    pub log_dir: String,
    pub cluster_file_path: String,
    pub config_path: String,
    pub primary_site: ProvisioningSite,
    pub secondary_site: ProvisioningSite,
    pub witness_site: ProvisioningSite,
    pub tenant_ids: Vec<String>,
    pub existing_coordinators: Vec<FoundationDbCoordinator>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbNodePlan {
    pub hostname: String,
    pub site: ProvisioningSite,
    pub role: ProvisioningFoundationDbRole,
    pub provider: InfrastructureProvider,
    pub instance_plan: ProvisioningInstancePlan,
    pub bootstrap: bool,
    pub coordinator: bool,
    pub data_bearing: bool,
    pub process_class: FoundationDbProcessClass,
    pub datacenter_id: String,
    pub zone_id: String,
    pub machine_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FoundationDbProcessClass {
    Storage,
    Coordinator,
    Stateless,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbCoordinator {
    pub hostname: String,
    pub site: ProvisioningSite,
    pub address_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbBackupPlan {
    pub enabled: bool,
    pub tag: String,
    pub destination_name: String,
    pub bucket_config_ref: String,
    pub access_key_secret_ref: ProvisioningSecretRef,
    pub secret_key_secret_ref: ProvisioningSecretRef,
    pub snapshot_interval_seconds: u32,
    pub initial_snapshot_interval_seconds: u32,
    pub server_side_encryption_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbRestorePlan {
    pub mode: FoundationDbRestoreMode,
    pub selector: Option<FoundationDbPlannerRestoreSelector>,
    pub source_config_ref: String,
    pub target_empty_confirmed: bool,
    pub force_live_restore: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbValidationError {
    pub reason: FoundationDbValidationErrorReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundationDbValidationErrorReason {
    InvalidAction,
    InvalidRole,
    InvalidIdentifier,
    InvalidPort,
    InvalidPath,
    MissingSiteBinding,
    MissingRequiredRole,
    MissingPeerCidrs,
    MissingSecretRef,
    MissingBackupConfig,
    InvalidBackupConfig,
    InvalidRestoreIntent,
    InvalidTopology,
    MissingExistingCluster,
    InvalidCoordinatorSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatsSuperclusterServiceSpec {
    pub clusters: Vec<NatsClusterDefinition>,
    pub jetstream_domains: Vec<JetstreamDomainDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatsClusterDefinition {
    pub cluster_id: String,
    pub data_region: ProvisioningDataRegion,
    pub sites: Vec<ProvisioningSite>,
    pub node_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JetstreamDomainDefinition {
    pub domain_id: String,
    pub data_region: ProvisioningDataRegion,
    pub cluster_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatsNodeDeploymentIntent {
    pub role: ProvisioningNatsRole,
    pub server_name: String,
    pub cluster_name: Option<String>,
    pub cluster_routes: Vec<NatsRouteTarget>,
    pub cluster_peer_count: u32,
    pub gateway_name: Option<String>,
    pub gateway_remotes: Vec<NatsGatewayRemote>,
    pub leafnode_listen_enabled: bool,
    pub leafnode_remotes: Vec<NatsRouteTarget>,
    pub jetstream_enabled: bool,
    pub jetstream_domain: Option<String>,
    pub store_dir: String,
    pub tls_enabled: bool,
    pub exporter_enabled: bool,
    pub private_peer_cidr_count: u32,
    pub admin_peer_cidr_count: u32,
    pub ports: NatsPortSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatsGatewayRemote {
    pub name: String,
    pub urls: Vec<NatsRouteTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatsRouteTarget {
    pub scheme: NatsRouteScheme,
    pub host: String,
    pub port: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NatsRouteScheme {
    Nats,
    Tls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatsPortSet {
    pub client: u32,
    pub cluster: u32,
    pub gateway: u32,
    pub leafnode: u32,
    pub monitor: u32,
    pub exporter: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatsDeploymentPlan {
    pub runtime_config: NatsRuntimeConfig,
    pub private_tcp_ports: Vec<u32>,
    pub health_probe: NatsHttpProbe,
    pub exporter_probe: Option<NatsHttpProbe>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatsRuntimeConfig {
    pub role: ProvisioningNatsRole,
    pub server_name: String,
    pub cluster_name: Option<String>,
    pub cluster_routes: Vec<NatsRouteTarget>,
    pub gateway_name: Option<String>,
    pub gateway_remotes: Vec<NatsGatewayRemote>,
    pub leafnode_listen_enabled: bool,
    pub leafnode_remotes: Vec<NatsRouteTarget>,
    pub jetstream_enabled: bool,
    pub jetstream_domain: Option<String>,
    pub store_dir: String,
    pub ports: NatsPortSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatsHttpProbe {
    pub port: u32,
    pub kind: NatsProbeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NatsProbeKind {
    Healthz,
    JetstreamHealthz,
    ExporterMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatsValidationError {
    pub reason: NatsValidationErrorReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NatsValidationErrorReason {
    InvalidIdentifier,
    InvalidPort,
    DuplicatePort,
    InvalidPath,
    InvalidRouteUrl,
    TlsRouteRequired,
    MissingPeerCidrs,
    MissingClusterName,
    MissingClusterRoutes,
    MissingGatewayRemotes,
    MissingGatewayRemoteUrls,
    MissingLeafnodeRemotes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityDeploymentIntent {
    pub workload_role: ProvisioningWorkloadRole,
    pub action: ProvisioningLifecycleAction,
    pub node_name: String,
    pub docker_host: bool,
    pub prometheus_enabled: bool,
    pub grafana_enabled: bool,
    pub hephaestus_enabled: bool,
    pub node_exporter_port: u32,
    pub cadvisor_port: u32,
    pub prometheus_port: u32,
    pub grafana_port: u32,
    pub hephaestus_http_port: u32,
    pub hephaestus_metrics_port: u32,
    pub compose_dir: String,
    pub prometheus_scrape_targets_artifact_path: String,
    pub prometheus_retention_time: String,
    pub prometheus_retention_size: String,
    pub scrape_peers: Vec<ObservabilityScrapePeer>,
    pub grafana_admin_password_secret_ref: ProvisioningSecretRef,
    pub grafana_secret_key_secret_ref: ProvisioningSecretRef,
    pub hephaestus_env_secret_ref: ProvisioningSecretRef,
    pub hephaestus_config_ref: String,
    pub private_peer_cidr_count: u32,
    pub admin_peer_cidr_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityScrapePeer {
    pub hostname: String,
    pub address: String,
    pub docker_host: bool,
    pub environment: ProvisioningEnvironment,
    pub data_region: ProvisioningDataRegion,
    pub site: ProvisioningSite,
    pub services: Vec<ProvisioningService>,
    pub placement_kind: ProvisioningPlacementKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityDeploymentPlan {
    pub runtime_config: ObservabilityRuntimeConfig,
    pub host_agents: Vec<ObservabilityHostAgentPlan>,
    pub scrape_jobs: Vec<PrometheusScrapeJobPlan>,
    pub probes: Vec<ObservabilityHttpProbe>,
    pub private_tcp_ports: Vec<u32>,
    pub required_secret_refs: Vec<ProvisioningSecretRef>,
    pub required_config_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityRuntimeConfig {
    pub workload_role: ProvisioningWorkloadRole,
    pub action: ProvisioningLifecycleAction,
    pub node_name: String,
    pub compose_dir: String,
    pub prometheus_scrape_targets_artifact_path: String,
    pub prometheus_retention_time: String,
    pub prometheus_retention_size: String,
    pub prometheus_enabled: bool,
    pub grafana_enabled: bool,
    pub hephaestus_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityHostAgentPlan {
    pub kind: ObservabilityHostAgentKind,
    pub port: u32,
    pub metrics_path: String,
    pub expected_health_fragment: String,
    pub private_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservabilityHostAgentKind {
    NodeExporter,
    Cadvisor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrometheusScrapeJobPlan {
    pub job_name: String,
    pub metrics_path: String,
    pub targets: Vec<PrometheusStaticTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrometheusStaticTarget {
    pub target: String,
    pub hostname: String,
    pub environment: ProvisioningEnvironment,
    pub data_region: ProvisioningDataRegion,
    pub site: ProvisioningSite,
    pub services: Vec<ProvisioningService>,
    pub metrics_source: ObservabilityHostAgentKind,
    pub placement_kind: ProvisioningPlacementKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityHttpProbe {
    pub port: u32,
    pub path: String,
    pub required_body_fragment: Option<String>,
    pub requires_secret: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityValidationError {
    pub reason: ObservabilityValidationErrorReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservabilityValidationErrorReason {
    InvalidRole,
    InvalidAction,
    InvalidIdentifier,
    InvalidPort,
    DuplicatePort,
    InvalidPath,
    MissingPeerCidrs,
    MissingScrapeTargets,
    MissingSecretRef,
    MissingHephaestusConfig,
    InvalidPeer,
    NoStackComponents,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseServiceSpec {
    pub replication_groups: Vec<TypesenseReplicationGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseReplicationGroup {
    pub group_id: String,
    pub data_regions: Vec<ProvisioningDataRegion>,
    pub replica_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseNodeDeploymentIntent {
    pub role: ProvisioningTypesenseRole,
    pub node_name: String,
    pub api_port: u32,
    pub peering_port: u32,
    pub cluster_peers: Vec<TypesenseClusterPeer>,
    pub peering_address: Option<String>,
    pub api_key_secret_ref: ProvisioningSecretRef,
    pub api_key_file_path: String,
    pub listen_address: String,
    pub data_dir: String,
    pub cors_enabled: bool,
    pub cors_domains: Vec<String>,
    pub tls: Option<TypesenseTlsConfig>,
    pub logging: Option<TypesenseLoggingConfig>,
    pub resource_guardrails: Option<TypesenseResourceGuardrails>,
    pub private_peer_cidr_count: u32,
    pub admin_peer_cidr_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseClusterPeer {
    pub address: String,
    pub peering_port: u32,
    pub api_port: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseTlsConfig {
    pub certificate_path: String,
    pub certificate_key_path: String,
    pub refresh_interval_seconds: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseLoggingConfig {
    pub log_dir: Option<String>,
    pub access_logging_enabled: bool,
    pub search_logging_enabled: bool,
    pub slow_requests_time_ms: Option<u32>,
    pub slow_searches_time_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseResourceGuardrails {
    pub memory_used_max_percentage: Option<u32>,
    pub disk_used_max_percentage: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseDeploymentPlan {
    pub runtime_config: TypesenseRuntimeConfig,
    pub private_tcp_ports: Vec<u32>,
    pub probes: Vec<TypesenseHttpProbe>,
    pub validation_collection_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseRuntimeConfig {
    pub role: ProvisioningTypesenseRole,
    pub node_name: String,
    pub api_port: u32,
    pub peering_port: u32,
    pub cluster_peers: Vec<TypesenseClusterPeer>,
    pub peering_address: Option<String>,
    pub api_key_secret_ref: ProvisioningSecretRef,
    pub api_key_file_path: String,
    pub listen_address: String,
    pub data_dir: String,
    pub cors_enabled: bool,
    pub cors_domains: Vec<String>,
    pub tls: Option<TypesenseTlsConfig>,
    pub logging: Option<TypesenseLoggingConfig>,
    pub resource_guardrails: Option<TypesenseResourceGuardrails>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseHttpProbe {
    pub port: u32,
    pub path: String,
    pub required_body_fragment: Option<String>,
    pub requires_api_key: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypesenseValidationError {
    pub reason: TypesenseValidationErrorReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypesenseValidationErrorReason {
    InvalidRole,
    InvalidIdentifier,
    InvalidPort,
    DuplicatePort,
    InvalidPeerAddress,
    InvalidPath,
    MissingSecretRef,
    MissingPeerCidrs,
    MissingPeeringAddress,
    UnexpectedPeeringAddress,
    MissingCorsDomains,
    InvalidTlsConfig,
    InvalidPercentage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanDockerDeploymentRequest {
    pub intent: DockerDeploymentIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanDockerDeploymentResponse {
    pub plan: Option<DockerDeploymentPlan>,
    pub errors: Vec<DockerValidationError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerDeploymentIntent {
    pub schema_version: u32,
    pub action: ProvisioningLifecycleAction,
    pub service: ProvisioningService,
    pub workload_roles: Vec<ProvisioningWorkloadRole>,
    pub runtime: ProvisioningRuntime,
    pub image: ContainerImageRef,
    pub container_name: String,
    pub compose_project_name: String,
    pub network_mode: DockerNetworkMode,
    pub restart_policy: DockerRestartPolicy,
    pub ports: Vec<DockerPortIntent>,
    pub volumes: Vec<DockerVolumeIntent>,
    pub secrets: Vec<DockerSecretMountIntent>,
    pub environment: Vec<DockerEnvironmentBinding>,
    pub probes: Vec<DockerHealthProbeIntent>,
    pub allow_public_ingress: bool,
    pub read_only_root_filesystem: bool,
    pub no_new_privileges: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerDeploymentPlan {
    pub runtime_config: DockerRuntimeConfig,
    pub image: DockerImagePlan,
    pub private_tcp_ports: Vec<u32>,
    pub public_tcp_ports: Vec<u32>,
    pub secret_mounts: Vec<DockerSecretMountIntent>,
    pub probes: Vec<DockerHealthProbeIntent>,
    pub operations: Vec<DockerPlannedOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerRuntimeConfig {
    pub service: ProvisioningService,
    pub workload_roles: Vec<ProvisioningWorkloadRole>,
    pub runtime: ProvisioningRuntime,
    pub container_name: String,
    pub compose_project_name: String,
    pub network_mode: DockerNetworkMode,
    pub restart_policy: DockerRestartPolicy,
    pub ports: Vec<DockerPortIntent>,
    pub volumes: Vec<DockerVolumeIntent>,
    pub environment: Vec<DockerEnvironmentBinding>,
    pub read_only_root_filesystem: bool,
    pub no_new_privileges: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImagePlan {
    pub image: ContainerImageRef,
    pub requires_registry_auth: bool,
    pub requires_local_archive_sync: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerPortIntent {
    pub name: String,
    pub container_port: u32,
    pub host_port: Option<u32>,
    pub protocol: DockerPortProtocol,
    pub public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerVolumeIntent {
    pub source: String,
    pub target: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerSecretMountIntent {
    pub secret_ref: ProvisioningSecretRef,
    pub target_path: String,
    pub env_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerEnvironmentBinding {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerHealthProbeIntent {
    pub name: String,
    pub kind: DockerHealthProbeKind,
    pub port: u32,
    pub path: Option<String>,
    pub required_body_fragment: Option<String>,
    pub requires_secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerPlannedOperation {
    pub action: ProvisioningLifecycleAction,
    pub phase: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockerNetworkMode {
    Bridge,
    Host,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockerRestartPolicy {
    No,
    UnlessStopped,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockerPortProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockerHealthProbeKind {
    Http,
    Tcp,
    Command,
    Metrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerValidationError {
    pub reason: DockerValidationErrorReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockerValidationErrorReason {
    UnsupportedSchemaVersion,
    InvalidAction,
    InvalidRuntime,
    InvalidImage,
    InvalidContainerName,
    InvalidPort,
    DuplicatePort,
    PublicPortRequiresPublicIngress,
    InvalidPath,
    MissingSecretRef,
    InvalidEnvironment,
    InvalidHealthProbe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationServiceSpec {
    pub runtime_kind: ApplicationRuntimeKind,
    pub image_repository: String,
    pub image_tag_policy: String,
    pub published_ports: Vec<ServicePort>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationRuntimeKind {
    NuxtSsr,
    RustApi,
    Graphql,
    GenericDockerService,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServicePort {
    pub name: String,
    pub port: u32,
    pub protocol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfrastructureProvider {
    Vultr,
    Cloudflare,
    Hetzner,
    Github,
    Local,
}

impl InfrastructureProvider {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Vultr => "vultr",
            Self::Cloudflare => "cloudflare",
            Self::Hetzner => "hetzner",
            Self::Github => "github",
            Self::Local => "local",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningRuntime {
    Docker,
    Host,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningLifecycleEngine {
    BaseNode,
    DockerHost,
    DockerService,
    Foundationdb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningLifecycleAction {
    Create,
    Configure,
    Status,
    Destroy,
    BootstrapHost,
    ConfigureService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningWorkloadRole {
    Api,
    Web,
    EdgeProxy,
    Observability,
    Ops,
    Graphql,
    Data,
}

impl ProvisioningWorkloadRole {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Web => "web",
            Self::EdgeProxy => "edge-proxy",
            Self::Observability => "observability",
            Self::Ops => "ops",
            Self::Graphql => "graphql",
            Self::Data => "data",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TailnetTagKind {
    #[serde(rename = "tag:api")]
    Api,
    #[serde(rename = "tag:web")]
    Web,
    #[serde(rename = "tag:nats")]
    Nats,
    #[serde(rename = "tag:nats-client")]
    NatsClient,
    #[serde(rename = "tag:typesense")]
    Typesense,
    #[serde(rename = "tag:edge")]
    Edge,
    #[serde(rename = "tag:observability")]
    Observability,
    #[serde(rename = "tag:ops")]
    Ops,
    #[serde(rename = "tag:staging")]
    Staging,
    #[serde(rename = "tag:region-eu")]
    RegionEu,
    #[serde(rename = "tag:region-us")]
    RegionUs,
    #[serde(rename = "tag:fdb-staging")]
    FdbStaging,
    #[serde(rename = "tag:fdb-eu")]
    FdbEu,
    #[serde(rename = "tag:fdb-us")]
    FdbUs,
    #[serde(rename = "tag:graphql-eu")]
    GraphqlEu,
    #[serde(rename = "tag:graphql-us")]
    GraphqlUs,
    #[serde(rename = "tag:data-eu")]
    DataEu,
    #[serde(rename = "tag:data-us")]
    DataUs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningHealthGroup {
    Host,
    Docker,
    Caddy,
    Nats,
    NatsExporter,
    Typesense,
    Foundationdb,
    Prometheus,
    Grafana,
    Api,
    Web,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockerServiceClassKind {
    EdgeApi,
    PrivateApi,
    Ops,
    BackgroundWorker,
    StatefulPrivateService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardEnvironmentRisk {
    NonProduction,
    Production,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardCreateStatus {
    LiveStagingSupported,
    LiveStagingSupportedRoleAware,
    LiveSupportedClusterMember,
    LiveStagingSupportedSingleNodeProdTopologyPending,
    StagingScriptExists,
    PlannedWaitingForPlatformImageContract,
    PlannedWaitingForWebImageContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardInputField {
    Service,
    Environment,
    Site,
    Role,
    Plan,
    ImageSource,
    ImageRef,
    IngressMode,
    HostImageSource,
    NodeSelector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionStrategy {
    SingleSite,
    SpreadAcrossSites,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkTransport {
    Vpc,
    Tailscale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkSource {
    PublicHttps,
    ApprovedAdminTailnetCidrs,
    AppNodes,
    OpsNodes,
    DeclaredServiceClients,
    ObservabilityNodes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServerTemplate {
    VultrDockerPrivateService,
    HetznerDockerPrivateService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutStrategy {
    ReplaceAllAfterApproval,
    RollingReplace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfrastructureEnvironment {
    pub environment_id: String,
    pub display_name: String,
    pub environment_type: String,
    pub jurisdiction: String,
    pub primary_site: Option<String>,
    pub server_type: String,
    pub topology_template: Option<String>,
    pub topology_summary: Option<String>,
    pub region_ids: Vec<String>,
    pub topology_document_paths: Vec<String>,
    pub tofu_directory: String,
    pub tofu_plan_path: String,
    pub generated_inventory_path: String,
    pub ansible_vars_paths: Vec<String>,
    pub ansible_playbook_paths: Vec<String>,
    pub allowed_action_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowActor {
    pub actor_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanInfraRequest {
    pub environment_id: String,
    pub requested_by: WorkflowActor,
    pub expected_git_commit: Option<String>,
    pub refresh_state: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanInfraResponse {
    pub run: WorkflowRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanNatsDeploymentRequest {
    pub intent: NatsNodeDeploymentIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanNatsDeploymentResponse {
    pub plan: Option<NatsDeploymentPlan>,
    pub errors: Vec<NatsValidationError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanObservabilityDeploymentRequest {
    pub intent: ObservabilityDeploymentIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanObservabilityDeploymentResponse {
    pub plan: Option<ObservabilityDeploymentPlan>,
    pub errors: Vec<ObservabilityValidationError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanTypesenseDeploymentRequest {
    pub intent: TypesenseNodeDeploymentIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanTypesenseDeploymentResponse {
    pub plan: Option<TypesenseDeploymentPlan>,
    pub errors: Vec<TypesenseValidationError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanVpsRequest {
    pub intent: VpsDeploymentIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanVpsResponse {
    pub plan: Option<VpsDeploymentPlan>,
    pub errors: Vec<VpsValidationError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpsDeploymentIntent {
    pub schema_version: u32,
    pub request_id: String,
    pub provider: InfrastructureProvider,
    pub environment: ProvisioningEnvironment,
    pub data_region: ProvisioningDataRegion,
    pub site: ProvisioningSite,
    pub service: ProvisioningService,
    pub placement_kind: ProvisioningPlacementKind,
    pub action: ProvisioningLifecycleAction,
    pub plan: ProvisioningInstancePlan,
    pub image: MachineImageRef,
    pub node_type: ProvisioningNodeType,
    pub ingress_mode: ProvisioningIngressMode,
    pub runtime: ProvisioningRuntime,
    pub hostname: String,
    pub provider_label: String,
    pub replica_ordinal: u32,
    pub temporary_public_bootstrap_ssh_cidrs: Vec<String>,
    pub admin_tailnet_cidrs: Vec<String>,
    pub private_tcp_ports: Vec<u32>,
    pub public_tcp_ports: Vec<u32>,
    pub provider_tags: Vec<String>,
    pub secret_refs: Vec<ProvisioningSecretRef>,
    pub tailnet_bootstrap: bool,
    pub allow_public_ingress: bool,
    pub nats_role: Option<ProvisioningNatsRole>,
    pub typesense_role: Option<ProvisioningTypesenseRole>,
    pub foundationdb_role: Option<ProvisioningFoundationDbRole>,
    pub workload_role: Option<ProvisioningWorkloadRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpsDeploymentPlan {
    pub runtime_config: VpsRuntimeConfig,
    pub provider_adapter: VpsProviderAdapterPlan,
    pub firewall: VpsFirewallPlan,
    pub bootstrap: VpsBootstrapPlan,
    pub operations: Vec<VpsPlannedOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpsRuntimeConfig {
    pub provider: InfrastructureProvider,
    pub environment: ProvisioningEnvironment,
    pub data_region: ProvisioningDataRegion,
    pub site: ProvisioningSite,
    pub service: ProvisioningService,
    pub placement_kind: ProvisioningPlacementKind,
    pub plan: ProvisioningInstancePlan,
    pub image: MachineImageRef,
    pub node_type: ProvisioningNodeType,
    pub ingress_mode: ProvisioningIngressMode,
    pub runtime: ProvisioningRuntime,
    pub hostname: String,
    pub provider_label: String,
    pub replica_ordinal: u32,
    pub provider_tags: Vec<String>,
    pub nats_role: Option<ProvisioningNatsRole>,
    pub typesense_role: Option<ProvisioningTypesenseRole>,
    pub foundationdb_role: Option<ProvisioningFoundationDbRole>,
    pub workload_role: Option<ProvisioningWorkloadRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpsProviderAdapterPlan {
    pub provider: InfrastructureProvider,
    pub provider_location_id: String,
    pub instance_plan_id: String,
    pub image_ref: String,
    pub os_id: String,
    pub provider_neutral: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpsFirewallPlan {
    pub temporary_public_bootstrap_ssh_cidrs: Vec<String>,
    pub admin_tailnet_cidrs: Vec<String>,
    pub private_tcp_ports: Vec<u32>,
    pub public_tcp_ports: Vec<u32>,
    pub public_ingress_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpsBootstrapPlan {
    pub tailnet_bootstrap: bool,
    pub required_secret_refs: Vec<ProvisioningSecretRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpsPlannedOperation {
    pub action: ProvisioningLifecycleAction,
    pub phase: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpsValidationError {
    pub reason: VpsValidationErrorReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpsValidationErrorReason {
    UnsupportedSchemaVersion,
    InvalidIdentifier,
    InvalidAction,
    InvalidRole,
    InvalidPort,
    InvalidCidr,
    MissingBootstrapCidr,
    MissingSecretRef,
    MissingImageRef,
    InvalidProviderPlan,
    InvalidNodeType,
    PublicPortRequiresPublicIngress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyInfraRequest {
    pub environment_id: String,
    pub plan_run_id: String,
    pub approved_by: WorkflowActor,
    pub expected_git_commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyInfraResponse {
    pub run: WorkflowRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub run_id: String,
    pub environment_id: String,
    pub kind: WorkflowRunKind,
    pub status: WorkflowRunStatus,
    pub requested_by: WorkflowActor,
    pub approved_by: Option<WorkflowActor>,
    pub created_at: HephaestusTimestamp,
    pub updated_at: HephaestusTimestamp,
    pub git_commit: Option<String>,
    pub plan_run_id: Option<String>,
    pub artifacts: Vec<WorkflowArtifact>,
    pub target: ServiceActionTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunKind {
    TofuPlan,
    TofuApply,
    Ansible,
    HealthCheck,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    WaitingForApproval,
    Cancelled,
    PreflightRunning,
    ConfirmationRequired,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowArtifact {
    pub kind: WorkflowArtifactKind,
    pub relative_path: String,
    pub redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookCatalog {
    pub schema_version: u32,
    pub catalog_id: String,
    pub description: String,
    pub environments: Vec<RunbookEnvironmentDefinition>,
    pub actions: Vec<RunbookActionDefinition>,
    pub monitoring_signals: Vec<RunbookMonitoringSignal>,
    pub catalog_references: Vec<RunbookCatalogReference>,
    pub execution_policy: Option<RunbookExecutionPolicy>,
    pub defaults: Option<RunbookCatalogDefaults>,
    pub risk_levels: Vec<RunbookRiskDefinition>,
    pub preflight_checks: Vec<RunbookPreflightCheckDefinition>,
    pub topology_templates: Vec<RunbookTopologyTemplate>,
    pub server_types: Vec<RunbookServerType>,
    pub server_templates: Vec<RunbookServerTemplate>,
    pub resource_templates: Vec<RunbookResourceTemplate>,
    pub region_policies: Vec<RunbookRegionPolicy>,
    pub lifecycle_model: Vec<String>,
    pub action_state_model: Vec<String>,
    pub dashboard_contract: Option<RunbookDashboardContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookCatalogDefaults {
    pub working_directory: String,
    pub command_model: String,
    pub operator_model: String,
    pub audit_log_required: bool,
    pub secret_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookRiskDefinition {
    pub risk_id: String,
    pub requires_confirmation: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookEnvironmentDefinition {
    pub environment_id: String,
    pub display_name: String,
    pub environment_type: String,
    pub jurisdiction: String,
    pub primary_site: Option<String>,
    pub server_type: String,
    pub topology_template: Option<String>,
    pub topology_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookActionDefinition {
    pub action_id: String,
    pub display_name: String,
    pub category: String,
    pub phase: String,
    pub risk: RunbookRiskLevel,
    pub supported_environment_ids: Vec<String>,
    pub operator_confirmation: Option<RunbookOperatorConfirmation>,
    pub preflight_checks: Vec<String>,
    pub success_indicators: Vec<String>,
    pub commands: Vec<RunbookEnvironmentCommand>,
    pub required_environment_variables: Vec<String>,
    pub required_files: Vec<RunbookEnvironmentRequiredFiles>,
    pub implementation_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookCatalogReference {
    pub catalog_kind: String,
    pub catalog_id: String,
    pub path: String,
    pub required_for: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookExecutionPolicy {
    pub no_free_form_shell: bool,
    pub resolve_executables_relative_to_repo_root: bool,
    pub run_with_empty_inherited_environment: bool,
    pub allowlist_environment_variables_only: bool,
    pub capture_stdout_stderr: bool,
    pub redact_output_before_display: bool,
    pub write_structured_audit_events: bool,
    pub per_environment_change_lock_required: bool,
    pub default_timeout_seconds: u32,
    pub max_timeout_seconds: u32,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookEnvironmentCommand {
    pub environment_id: String,
    pub command: RunbookCommandSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookCommandSpec {
    pub executable: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookEnvironmentRequiredFiles {
    pub environment_id: String,
    pub relative_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunbookRiskLevel {
    Read,
    Change,
    Destructive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookOperatorConfirmation {
    pub style: RunbookConfirmationStyle,
    pub typed_text: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunbookConfirmationStyle {
    DashboardConfirm,
    Typed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookMonitoringSignal {
    pub signal_id: String,
    pub applies_to_server_types: Vec<String>,
    pub checks: Vec<String>,
    pub dashboard_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookPreflightCheckDefinition {
    pub preflight_id: String,
    pub category: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookTopologyTemplate {
    pub template_id: String,
    pub display_name: String,
    pub description: String,
    pub allowed_environment_types: Vec<String>,
    pub region_strategy: String,
    pub site_roles: Vec<RunbookTopologySiteRole>,
    pub server_groups: Vec<RunbookTopologyServerGroup>,
    pub database_policy: Option<RunbookDatabasePolicy>,
    pub resources: Vec<String>,
    pub image_selection: Option<RunbookImageSelectionPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookTopologySiteRole {
    pub role: String,
    pub min_sites: u32,
    pub max_sites: u32,
    pub data_bearing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookTopologyServerGroup {
    pub name: String,
    pub server_type: String,
    pub min_count: u32,
    pub default_count: u32,
    pub max_count: Option<u32>,
    pub resize_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookDatabasePolicy {
    pub redundancy_mode: String,
    pub storage_engine: String,
    pub usable_regions: u32,
    pub backup_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookImageSelectionPolicy {
    pub catalog: String,
    pub allowed_image_source_types: Vec<String>,
    pub production_requires_digest: bool,
    pub production_requires_sbom: bool,
    pub production_requires_provenance: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookServerType {
    pub server_type_id: String,
    pub display_name: String,
    pub runtime: String,
    pub base_role: String,
    #[serde(alias = "service_role")]
    pub service_identity_label: Option<String>,
    pub dockerized: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookServerTemplate {
    pub template_id: String,
    pub server_type: String,
    pub provider: String,
    pub os: String,
    pub default_plan: String,
    pub admin_access: String,
    pub storage_policy: String,
    pub public_ingress: Vec<RunbookIngressRule>,
    pub private_ingress: Vec<RunbookIngressRule>,
    pub base_roles: Vec<String>,
    #[serde(alias = "service_roles")]
    pub service_identity_labels: Vec<String>,
    pub service_contract_required: Vec<String>,
    pub scaling_policy: Option<RunbookScalingPolicy>,
    pub image_selection_policy: Option<RunbookServerImageSelectionPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookIngressRule {
    pub name: String,
    pub port: u32,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookScalingPolicy {
    pub supports_in_place_resize: bool,
    pub supports_additive_scale_out: bool,
    pub requires_database_rebalance_check: bool,
    pub operator_notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookServerImageSelectionPolicy {
    pub provider_catalog: String,
    pub image_catalog: String,
    pub allowed_compute_image_sources: Vec<String>,
    pub allowed_container_image_sources: Vec<String>,
    pub production_requires_immutable_digest: bool,
    pub dashboard_must_display_image_apps: bool,
    pub dashboard_must_display_exposed_ports: bool,
    pub dashboard_must_display_volume_policy: bool,
    pub dashboard_must_display_security_metadata: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookResourceTemplate {
    pub resource_id: String,
    pub provider: String,
    pub resource_type: String,
    pub lifecycle: String,
    pub region_scope: Option<String>,
    pub default_public_ingress: Vec<RunbookIngressRule>,
    pub bootstrap_public_ingress: Vec<RunbookProtocolIngressRule>,
    pub requirements: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookProtocolIngressRule {
    pub protocol: String,
    pub port: u32,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookRegionPolicy {
    pub policy_id: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookDashboardContract {
    pub button_enablement: Vec<String>,
    pub future_docker_services: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowArtifactKind {
    GitDiff,
    TofuPlanBinary,
    TofuPlanText,
    TofuOutputJson,
    AnsibleLog,
    HealthReport,
    RecoveryReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceActionTarget {
    pub service_id: Option<String>,
    pub service_group_id: Option<String>,
    pub service_kind: ServiceKind,
    pub region_id: Option<String>,
    pub site_id: Option<String>,
    pub tenant_id: Option<String>,
    pub server_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowOperationKind {
    ConfigureService,
    InitializeService,
    FinalizeService,
    DeployService,
    ShowStatus,
    StartBackup,
    CheckBackup,
    RestoreBackup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceOperationOptions {
    pub foundationdb_restore_backup: Option<FoundationDbRestoreBackupOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunAnsibleRequest {
    pub environment_id: String,
    pub target: ServiceActionTarget,
    pub operation: WorkflowOperationKind,
    pub requested_by: WorkflowActor,
    pub options: Option<ServiceOperationOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunAnsibleResponse {
    pub run: WorkflowRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundationDbRestoreBackupOptions {
    pub mode: RestoreExecutionMode,
    pub selector: Option<FoundationDbRestoreSelector>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FoundationDbRestoreSelector {
    Timestamp(String),
    Version(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreExecutionMode {
    DryRun,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckTarget {
    pub target: ServiceActionTarget,
    pub kind: HealthCheckKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthCheckKind {
    ProviderState,
    HostState,
    ServiceStatus,
    ReplicationStatus,
    FailoverReadiness,
    BackupStatus,
    EndpointProbe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunHealthChecksRequest {
    pub environment_id: String,
    pub checks: Vec<HealthCheckTarget>,
    pub requested_by: WorkflowActor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunHealthChecksResponse {
    pub run: WorkflowRun,
    pub snapshot: Option<HealthSnapshot>,
    pub infrastructure_snapshot: Option<InfrastructureHealthSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRunbookActionRequest {
    pub environment_id: String,
    pub action_id: String,
    pub requested_by: WorkflowActor,
    pub confirmation_accepted: bool,
    pub confirmation_text: Option<String>,
    pub secret_refs: Vec<String>,
    pub dry_run: bool,
    pub plan_run_id: Option<String>,
    pub actor_role_ids: Vec<String>,
    pub approvals: Vec<RunbookActionApproval>,
    pub break_glass_incident_id: Option<String>,
    pub provisioning_request: Option<ProvisioningRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRunbookActionResponse {
    pub run: WorkflowRun,
    pub infrastructure_snapshot: Option<InfrastructureHealthSnapshot>,
    pub provisioning_plan: Option<ProvisioningPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub action: ProvisioningAction,
    pub service: ProvisioningService,
    pub environment: ProvisioningEnvironment,
    pub site: ProvisioningSite,
    pub data_region: ProvisioningDataRegion,
    pub plan: ProvisioningInstancePlan,
    pub machine_image: Option<MachineImageRef>,
    pub ingress_mode: ProvisioningIngressMode,
    pub replicas: u32,
    pub allow_replace: bool,
    pub allow_public_ingress: bool,
    pub secret_refs: Vec<ProvisioningSecretRef>,
    pub dry_run: bool,
    pub nats_role: Option<ProvisioningNatsRole>,
    pub typesense_role: Option<ProvisioningTypesenseRole>,
    pub provider: InfrastructureProvider,
    pub node_type: ProvisioningNodeType,
    pub os: ProvisioningOs,
    pub placement_kind: ProvisioningPlacementKind,
    pub idempotency_key: String,
    pub foundationdb_role: Option<ProvisioningFoundationDbRole>,
    pub workload_role: Option<ProvisioningWorkloadRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineImageRef {
    pub source: ProvisioningMachineImageSource,
    pub image_ref: String,
    pub digest: Option<String>,
    pub os: ProvisioningOs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerImageRef {
    pub source: ProvisioningContainerImageSource,
    pub registry: String,
    pub repository: String,
    pub tag: String,
    pub digest: Option<String>,
    pub auth_secret_ref: Option<ProvisioningSecretRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningSecretRef {
    pub id: String,
    pub secret_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningPlan {
    pub schema_version: u32,
    pub request_id: String,
    pub action: ProvisioningAction,
    pub supported: bool,
    pub resolution_status: ProvisioningResolutionStatus,
    pub dry_run: bool,
    pub derived: ProvisioningPlanDerived,
    pub execution: ProvisioningPlanExecution,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningPlanDerived {
    pub provider: String,
    pub provider_environment_id: Option<String>,
    pub service: ProvisioningService,
    pub environment: ProvisioningEnvironment,
    pub site: ProvisioningSite,
    pub provider_location: String,
    pub data_region: ProvisioningDataRegion,
    pub plan: ProvisioningInstancePlan,
    pub replicas: u32,
    pub ingress_mode: ProvisioningIngressMode,
    pub public_ingress: bool,
    pub tailscale_tags: Vec<TailnetTagKind>,
    pub provider_tags: Vec<String>,
    pub service_port: Option<u32>,
    pub machine_image: Option<MachineImageRef>,
    pub owner_repository: Option<String>,
    pub source_repository: Option<ProvisioningSourceRepository>,
    pub nats_role: Option<ProvisioningNatsRole>,
    pub typesense_role: Option<ProvisioningTypesenseRole>,
    pub provider_kind: InfrastructureProvider,
    pub node_type: ProvisioningNodeType,
    pub os: ProvisioningOs,
    pub placement_kind: ProvisioningPlacementKind,
    pub reconciliation_status: ProvisioningReconciliationStatus,
    pub idempotency_key: String,
    pub foundationdb_role: Option<ProvisioningFoundationDbRole>,
    pub workload_role: Option<ProvisioningWorkloadRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningSourceRepository {
    pub id: String,
    pub kind: String,
    pub owner: String,
    pub repository: String,
    pub owns_services: Vec<String>,
    pub artifact_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningPlanExecution {
    pub mode: ProvisioningExecutionMode,
    pub execute_via_shell: bool,
    pub allow_replace: bool,
    pub required_secret_bindings: Vec<ProvisioningRequiredSecretBinding>,
    pub commands: Vec<ProvisioningCommand>,
    pub unsupported_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningRequiredSecretBinding {
    pub id: String,
    pub env_var: String,
    pub secret_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningCommand {
    pub phase: String,
    pub executable: String,
    pub argv: Vec<String>,
    pub env: Vec<ProvisioningCommandEnvBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningCommandEnvBinding {
    pub env_var: String,
    pub from_secret_ref: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningAction {
    CreateServer,
}

impl ProvisioningAction {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::CreateServer => "create_server",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningService {
    Api,
    Web,
    Nats,
    Typesense,
    CaddyEdge,
    Fdb,
    Observability,
}

impl ProvisioningService {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Web => "web",
            Self::Nats => "nats",
            Self::Typesense => "typesense",
            Self::CaddyEdge => "caddy-edge",
            Self::Fdb => "fdb",
            Self::Observability => "observability",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningEnvironment {
    Staging,
    Prod,
    Dev,
}

impl ProvisioningEnvironment {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Staging => "staging",
            Self::Prod => "prod",
            Self::Dev => "dev",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningDataRegion {
    Eu,
    Us,
    None,
}

impl ProvisioningDataRegion {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Eu => "eu",
            Self::Us => "us",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningPlacementKind {
    Regional,
    Edge,
}

impl ProvisioningPlacementKind {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Regional => "regional",
            Self::Edge => "edge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProvisioningSite {
    Ams,
    Ash,
    Atl,
    Blr,
    Bom,
    Cdg,
    Del,
    Dfw,
    Ewr,
    Fra,
    Gru,
    Hel,
    Hnl,
    Iad,
    Icn,
    Itm,
    Jnb,
    Lax,
    Lej,
    Lhr,
    Mad,
    Man,
    Mel,
    Mex,
    Mia,
    Mxp,
    Nrt,
    Nue,
    Ord,
    Pdx,
    Scl,
    Sea,
    Sin,
    Sjc,
    Sto,
    Syd,
    Tlv,
    Waw,
    Yto,
}

impl ProvisioningSite {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Ams => "ams",
            Self::Ash => "ash",
            Self::Atl => "atl",
            Self::Blr => "blr",
            Self::Bom => "bom",
            Self::Cdg => "cdg",
            Self::Del => "del",
            Self::Dfw => "dfw",
            Self::Ewr => "ewr",
            Self::Fra => "fra",
            Self::Gru => "gru",
            Self::Hel => "hel",
            Self::Hnl => "hnl",
            Self::Iad => "iad",
            Self::Icn => "icn",
            Self::Itm => "itm",
            Self::Jnb => "jnb",
            Self::Lax => "lax",
            Self::Lej => "lej",
            Self::Lhr => "lhr",
            Self::Mad => "mad",
            Self::Man => "man",
            Self::Mel => "mel",
            Self::Mex => "mex",
            Self::Mia => "mia",
            Self::Mxp => "mxp",
            Self::Nrt => "nrt",
            Self::Nue => "nue",
            Self::Ord => "ord",
            Self::Pdx => "pdx",
            Self::Scl => "scl",
            Self::Sea => "sea",
            Self::Sin => "sin",
            Self::Sjc => "sjc",
            Self::Sto => "sto",
            Self::Syd => "syd",
            Self::Tlv => "tlv",
            Self::Waw => "waw",
            Self::Yto => "yto",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderLocationCapability {
    Compute,
    LoadBalancer,
    Kubernetes,
    BlockStorage,
    NatGateway,
    PrivateNetwork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderServerCapability {
    CreateServer,
    DeleteServer,
    GetServer,
    ListPlans,
    ListLocations,
    ListImages,
    AttachFirewall,
    AttachNetwork,
    RenderCloudInit,
    SnapshotGoldenImage,
    RegistryAuth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderLocationMapping {
    pub provider: InfrastructureProvider,
    pub provider_location_id: String,
    pub site: ProvisioningSite,
    pub data_region: ProvisioningDataRegion,
    pub placement_kind: ProvisioningPlacementKind,
    pub city: String,
    pub country: String,
    pub provider_network_zone: String,
    pub capabilities: Vec<ProviderLocationCapability>,
    pub canonical: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderLocationMatrix {
    pub locations: Vec<ProviderLocationMapping>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticProviderLocationMapping {
    pub provider: InfrastructureProvider,
    pub provider_location_id: &'static str,
    pub site: ProvisioningSite,
    pub data_region: ProvisioningDataRegion,
    pub placement_kind: ProvisioningPlacementKind,
    pub city: &'static str,
    pub country: &'static str,
    pub provider_network_zone: &'static str,
    pub capabilities: &'static [ProviderLocationCapability],
    pub canonical: bool,
    pub notes: Option<&'static str>,
}

impl StaticProviderLocationMapping {
    pub fn to_owned_mapping(self) -> ProviderLocationMapping {
        ProviderLocationMapping {
            provider: self.provider,
            provider_location_id: self.provider_location_id.to_owned(),
            site: self.site,
            data_region: self.data_region,
            placement_kind: self.placement_kind,
            city: self.city.to_owned(),
            country: self.country.to_owned(),
            provider_network_zone: self.provider_network_zone.to_owned(),
            capabilities: self.capabilities.to_vec(),
            canonical: self.canonical,
            notes: self.notes.map(str::to_owned),
        }
    }
}

const VULTR_COMPUTE_LB_K8S_NAT: &[ProviderLocationCapability] = &[
    ProviderLocationCapability::Compute,
    ProviderLocationCapability::LoadBalancer,
    ProviderLocationCapability::Kubernetes,
    ProviderLocationCapability::BlockStorage,
    ProviderLocationCapability::NatGateway,
    ProviderLocationCapability::PrivateNetwork,
];

const VULTR_COMPUTE_K8S_NAT: &[ProviderLocationCapability] = &[
    ProviderLocationCapability::Compute,
    ProviderLocationCapability::Kubernetes,
    ProviderLocationCapability::BlockStorage,
    ProviderLocationCapability::NatGateway,
    ProviderLocationCapability::PrivateNetwork,
];

const HETZNER_CLOUD_ALL: &[ProviderLocationCapability] = &[
    ProviderLocationCapability::Compute,
    ProviderLocationCapability::LoadBalancer,
    ProviderLocationCapability::BlockStorage,
    ProviderLocationCapability::PrivateNetwork,
];

pub const PROVIDER_LOCATION_MATRIX: &[StaticProviderLocationMapping] = &[
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "ams",
        site: ProvisioningSite::Ams,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Amsterdam",
        country: "NL",
        provider_network_zone: "Europe",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "atl",
        site: ProvisioningSite::Atl,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Atlanta",
        country: "US",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "blr",
        site: ProvisioningSite::Blr,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Bangalore",
        country: "IN",
        provider_network_zone: "Asia",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "bom",
        site: ProvisioningSite::Bom,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Mumbai",
        country: "IN",
        provider_network_zone: "Asia",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "cdg",
        site: ProvisioningSite::Cdg,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Paris",
        country: "FR",
        provider_network_zone: "Europe",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "del",
        site: ProvisioningSite::Del,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Delhi NCR",
        country: "IN",
        provider_network_zone: "Asia",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "dfw",
        site: ProvisioningSite::Dfw,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Dallas",
        country: "US",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "ewr",
        site: ProvisioningSite::Ewr,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "New Jersey",
        country: "US",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "fra",
        site: ProvisioningSite::Fra,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Frankfurt",
        country: "DE",
        provider_network_zone: "Europe",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "hnl",
        site: ProvisioningSite::Hnl,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Honolulu",
        country: "US",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "icn",
        site: ProvisioningSite::Icn,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Seoul",
        country: "KR",
        provider_network_zone: "Asia",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "itm",
        site: ProvisioningSite::Itm,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Osaka",
        country: "JP",
        provider_network_zone: "Asia",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "jnb",
        site: ProvisioningSite::Jnb,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Johannesburg",
        country: "ZA",
        provider_network_zone: "Africa",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "lax",
        site: ProvisioningSite::Lax,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Los Angeles",
        country: "US",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "lhr",
        site: ProvisioningSite::Lhr,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "London",
        country: "GB",
        provider_network_zone: "Europe",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "mad",
        site: ProvisioningSite::Mad,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Madrid",
        country: "ES",
        provider_network_zone: "Europe",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "man",
        site: ProvisioningSite::Man,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Manchester",
        country: "GB",
        provider_network_zone: "Europe",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "mel",
        site: ProvisioningSite::Mel,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Melbourne",
        country: "AU",
        provider_network_zone: "Australia",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "mex",
        site: ProvisioningSite::Mex,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Mexico City",
        country: "MX",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "mia",
        site: ProvisioningSite::Mia,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Miami",
        country: "US",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "mxp",
        site: ProvisioningSite::Mxp,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Milan",
        country: "IT",
        provider_network_zone: "Europe",
        capabilities: VULTR_COMPUTE_K8S_NAT,
        canonical: true,
        notes: Some(
            "Vultr reports no load_balancers option for this location as of the current regions API response.",
        ),
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "nrt",
        site: ProvisioningSite::Nrt,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Tokyo",
        country: "JP",
        provider_network_zone: "Asia",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "ord",
        site: ProvisioningSite::Ord,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Chicago",
        country: "US",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "sao",
        site: ProvisioningSite::Gru,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Sao Paulo",
        country: "BR",
        provider_network_zone: "South America",
        capabilities: VULTR_COMPUTE_K8S_NAT,
        canonical: false,
        notes: Some("Provider id is sao; ReallyMe site uses GRU airport code."),
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "scl",
        site: ProvisioningSite::Scl,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Santiago",
        country: "CL",
        provider_network_zone: "South America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "sea",
        site: ProvisioningSite::Sea,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Seattle",
        country: "US",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "sgp",
        site: ProvisioningSite::Sin,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Singapore",
        country: "SG",
        provider_network_zone: "Asia",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: false,
        notes: Some("Provider id is sgp; ReallyMe site uses SIN airport code."),
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "sjc",
        site: ProvisioningSite::Sjc,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Silicon Valley",
        country: "US",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "sto",
        site: ProvisioningSite::Sto,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Stockholm",
        country: "SE",
        provider_network_zone: "Europe",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "syd",
        site: ProvisioningSite::Syd,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Sydney",
        country: "AU",
        provider_network_zone: "Australia",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "tlv",
        site: ProvisioningSite::Tlv,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Tel Aviv",
        country: "IL",
        provider_network_zone: "Asia",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "waw",
        site: ProvisioningSite::Waw,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Warsaw",
        country: "PL",
        provider_network_zone: "Europe",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Vultr,
        provider_location_id: "yto",
        site: ProvisioningSite::Yto,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Toronto",
        country: "CA",
        provider_network_zone: "North America",
        capabilities: VULTR_COMPUTE_LB_K8S_NAT,
        canonical: true,
        notes: None,
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Hetzner,
        provider_location_id: "fsn1",
        site: ProvisioningSite::Lej,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Falkenstein",
        country: "DE",
        provider_network_zone: "eu-central",
        capabilities: HETZNER_CLOUD_ALL,
        canonical: false,
        notes: Some("Falkenstein maps to the Leipzig-area airport code LEJ."),
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Hetzner,
        provider_location_id: "nbg1",
        site: ProvisioningSite::Nue,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Nuremberg",
        country: "DE",
        provider_network_zone: "eu-central",
        capabilities: HETZNER_CLOUD_ALL,
        canonical: false,
        notes: Some("Provider id nbg1 maps to airport code NUE."),
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Hetzner,
        provider_location_id: "hel1",
        site: ProvisioningSite::Hel,
        data_region: ProvisioningDataRegion::Eu,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Helsinki",
        country: "FI",
        provider_network_zone: "eu-central",
        capabilities: HETZNER_CLOUD_ALL,
        canonical: false,
        notes: Some("Provider id hel1 maps to airport code HEL."),
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Hetzner,
        provider_location_id: "ash",
        site: ProvisioningSite::Iad,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Ashburn",
        country: "US",
        provider_network_zone: "us-east",
        capabilities: HETZNER_CLOUD_ALL,
        canonical: false,
        notes: Some("Provider id ash maps to the Washington Dulles airport code IAD."),
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Hetzner,
        provider_location_id: "hil",
        site: ProvisioningSite::Pdx,
        data_region: ProvisioningDataRegion::Us,
        placement_kind: ProvisioningPlacementKind::Regional,
        city: "Hillsboro",
        country: "US",
        provider_network_zone: "us-west",
        capabilities: HETZNER_CLOUD_ALL,
        canonical: false,
        notes: Some("Provider id hil maps to the Portland-area airport code PDX."),
    },
    StaticProviderLocationMapping {
        provider: InfrastructureProvider::Hetzner,
        provider_location_id: "sin",
        site: ProvisioningSite::Sin,
        data_region: ProvisioningDataRegion::None,
        placement_kind: ProvisioningPlacementKind::Edge,
        city: "Singapore",
        country: "SG",
        provider_network_zone: "ap-southeast",
        capabilities: HETZNER_CLOUD_ALL,
        canonical: true,
        notes: None,
    },
];

pub fn provider_location_matrix() -> ProviderLocationMatrix {
    ProviderLocationMatrix {
        locations: PROVIDER_LOCATION_MATRIX
            .iter()
            .copied()
            .map(StaticProviderLocationMapping::to_owned_mapping)
            .collect(),
    }
}

pub fn provider_location_mapping(
    provider: InfrastructureProvider,
    provider_location_id: &str,
) -> Option<&'static StaticProviderLocationMapping> {
    PROVIDER_LOCATION_MATRIX.iter().find(|mapping| {
        mapping.provider == provider && mapping.provider_location_id == provider_location_id
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningIngressMode {
    #[serde(rename = "vultr-lb")]
    VultrLoadBalancer,
    #[serde(rename = "hetzner-lb")]
    HetznerLoadBalancer,
    Caddy,
    InternalOnly,
}

impl ProvisioningIngressMode {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::VultrLoadBalancer => "vultr-lb",
            Self::HetznerLoadBalancer => "hetzner-lb",
            Self::Caddy => "caddy",
            Self::InternalOnly => "internal-only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningNodeType {
    Vps,
    LoadBalancer,
}

impl ProvisioningNodeType {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Vps => "vps",
            Self::LoadBalancer => "load-balancer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningNatsRole {
    Regional,
    Gateway,
    EdgeLeaf,
}

impl ProvisioningNatsRole {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Regional => "regional",
            Self::Gateway => "gateway",
            Self::EdgeLeaf => "edge-leaf",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningTypesenseRole {
    ClusterMember,
}

impl ProvisioningTypesenseRole {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::ClusterMember => "cluster-member",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningMachineImageSource {
    #[serde(rename = "os")]
    ProviderOs,
    ProviderImage,
    ProviderSnapshot,
    GoldenImage,
}

impl ProvisioningMachineImageSource {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::ProviderOs => "os",
            Self::ProviderImage => "provider-image",
            Self::ProviderSnapshot => "provider-snapshot",
            Self::GoldenImage => "golden-image",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningContainerImageSource {
    GithubContainerRegistry,
    VultrContainerRegistry,
    HetznerContainerRegistry,
    LocalArchive,
}

impl ProvisioningContainerImageSource {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::GithubContainerRegistry => "github-container-registry",
            Self::VultrContainerRegistry => "vultr-container-registry",
            Self::HetznerContainerRegistry => "hetzner-container-registry",
            Self::LocalArchive => "local-archive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningOs {
    #[serde(rename = "ubuntu-26.04-lts", alias = "Ubuntu 26.04 LTS x64")]
    Ubuntu2604Lts,
}

impl ProvisioningOs {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Ubuntu2604Lts => "ubuntu-26.04-lts",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningInstancePlan {
    #[serde(rename = "vc2-1c-1gb")]
    VultrVc2OneCoreOneGb,
    #[serde(rename = "vc2-2c-4gb")]
    VultrVc2TwoCoreFourGb,
    #[serde(rename = "vc2-4c-8gb")]
    VultrVc2FourCoreEightGb,
    #[serde(rename = "cx23")]
    HetznerCx23,
    #[serde(rename = "cx33")]
    HetznerCx33,
    #[serde(rename = "cx43")]
    HetznerCx43,
    #[serde(rename = "cx53")]
    HetznerCx53,
    #[serde(rename = "cax11")]
    HetznerCax11,
    #[serde(rename = "cax21")]
    HetznerCax21,
    #[serde(rename = "cpx22")]
    HetznerCpx22,
    #[serde(rename = "cpx32")]
    HetznerCpx32,
    #[serde(rename = "cpx42")]
    HetznerCpx42,
    #[serde(rename = "cpx52")]
    HetznerCpx52,
    #[serde(rename = "cpx62")]
    HetznerCpx62,
    #[serde(rename = "ccx13")]
    HetznerCcx13,
    #[serde(rename = "ccx23")]
    HetznerCcx23,
    #[serde(rename = "ccx33")]
    HetznerCcx33,
    #[serde(rename = "ccx43")]
    HetznerCcx43,
    #[serde(rename = "ccx53")]
    HetznerCcx53,
    #[serde(rename = "ccx63")]
    HetznerCcx63,
}

impl ProvisioningInstancePlan {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::VultrVc2OneCoreOneGb => "vc2-1c-1gb",
            Self::VultrVc2TwoCoreFourGb => "vc2-2c-4gb",
            Self::VultrVc2FourCoreEightGb => "vc2-4c-8gb",
            Self::HetznerCx23 => "cx23",
            Self::HetznerCx33 => "cx33",
            Self::HetznerCx43 => "cx43",
            Self::HetznerCx53 => "cx53",
            Self::HetznerCax11 => "cax11",
            Self::HetznerCax21 => "cax21",
            Self::HetznerCpx22 => "cpx22",
            Self::HetznerCpx32 => "cpx32",
            Self::HetznerCpx42 => "cpx42",
            Self::HetznerCpx52 => "cpx52",
            Self::HetznerCpx62 => "cpx62",
            Self::HetznerCcx13 => "ccx13",
            Self::HetznerCcx23 => "ccx23",
            Self::HetznerCcx33 => "ccx33",
            Self::HetznerCcx43 => "ccx43",
            Self::HetznerCcx53 => "ccx53",
            Self::HetznerCcx63 => "ccx63",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningResolutionStatus {
    Ready,
    Planned,
    Invalid,
}

impl ProvisioningResolutionStatus {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Planned => "planned",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvisioningReconciliationStatus {
    NotStarted,
    Requested,
    ProviderObserved,
    TailnetObserved,
    Healthy,
    Drifted,
    Failed,
}

impl ProvisioningReconciliationStatus {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not-started",
            Self::Requested => "requested",
            Self::ProviderObserved => "provider-observed",
            Self::TailnetObserved => "tailnet-observed",
            Self::Healthy => "healthy",
            Self::Drifted => "drifted",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningExecutionMode {
    ArgvOnly,
}

impl ProvisioningExecutionMode {
    pub const fn as_catalog_str(self) -> &'static str {
        match self {
            Self::ArgvOnly => "argv_only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookActionApproval {
    pub actor_id: String,
    pub role_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfrastructureActionRequest {
    pub schema_version: u32,
    pub environment_id: String,
    pub action_id: String,
    pub actor_id: String,
    pub confirmation: Option<InfrastructureActionConfirmation>,
    pub secret_refs: Vec<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfrastructureActionConfirmation {
    pub typed_text: Option<String>,
    pub accepted: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfrastructureActionEvent {
    pub schema_version: u32,
    pub event_type: InfrastructureActionEventType,
    pub invocation_id: String,
    pub environment_id: String,
    pub action_id: String,
    pub actor_id: String,
    pub created_at: HephaestusTimestamp,
    pub exit_code: Option<u32>,
    pub redacted_summary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfrastructureActionEventType {
    ActionQueued,
    PreflightStarted,
    PreflightFailed,
    ConfirmationRequired,
    ActionStarted,
    ActionSucceeded,
    ActionFailed,
    ActionCanceled,
    ActionTimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfrastructureJsonSchemaDocument {
    pub relative_path: String,
    pub schema_id: String,
    pub title: String,
    pub required_fields: Vec<String>,
    pub property_names: Vec<String>,
    pub schema_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfrastructureOperationalDocument {
    pub relative_path: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageCatalog {
    pub schema_version: u32,
    pub catalog_id: String,
    pub catalog_kind: String,
    pub source_policy: DockerImageSourcePolicy,
    pub service_classes: Vec<DockerServiceClass>,
    pub image_metadata_schema: DockerImageMetadataSchema,
    pub images: Vec<DockerServiceImage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageSourcePolicy {
    pub image_digest_required: bool,
    #[serde(default)]
    pub platform_metadata_required: bool,
    pub floating_tags_disallowed_for_production: bool,
    pub sbom_reference_required_for_production: bool,
    pub provenance_reference_required_for_production: bool,
    pub operator_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerServiceClass {
    #[serde(rename = "id")]
    pub service_class_id: String,
    pub display_name: String,
    pub default_public_ingress: Vec<u32>,
    pub requires_load_balancer_review: bool,
    pub allowed_network_sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageMetadataSchema {
    pub required_fields: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerServiceImage {
    #[serde(rename = "id")]
    pub image_id: String,
    pub display_name: String,
    pub implementation_status: Option<String>,
    pub deployment_eligibility: Option<String>,
    pub service_class: String,
    pub registry: DockerImageRegistry,
    #[serde(default)]
    pub supported_platforms: Vec<DockerImageSupportedPlatform>,
    #[serde(default)]
    pub build: DockerImageBuildMetadata,
    pub apps: Vec<DockerImageApp>,
    pub ports: Vec<DockerImagePort>,
    pub volumes: Vec<DockerImageVolume>,
    pub health_checks: Vec<DockerImageHealthCheck>,
    #[serde(default)]
    pub observability: DockerImageObservability,
    pub secrets: Vec<DockerImageSecret>,
    pub backup_policy: DockerImageBackupPolicy,
    pub region_constraints: DockerImageRegionConstraints,
    pub resource_defaults: DockerImageResourceDefaults,
    pub security: DockerImageSecurityPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageSupportedPlatform {
    pub os: String,
    pub architecture: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageBuildMetadata {
    pub dockerfile_path: String,
    pub build_context_path: String,
    pub feature_flags: Vec<String>,
    pub local_image_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageRegistry {
    pub provider: String,
    pub hostname: String,
    pub repository: String,
    pub tag: String,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageApp {
    pub name: String,
    pub kind: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImagePort {
    pub name: String,
    pub container_port: u32,
    pub host_port: Option<u32>,
    pub protocol: String,
    pub exposure: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageVolume {
    pub name: String,
    pub mount_path: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    pub required: bool,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageHealthCheck {
    pub name: String,
    #[serde(rename = "type")]
    pub check_type: String,
    pub path: Option<String>,
    pub port: Option<u32>,
    pub expected_status: Option<u32>,
    pub command_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageSecret {
    pub name: String,
    pub source: String,
    pub required: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageObservability {
    pub metrics_endpoints: Vec<String>,
    pub structured_log_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageBackupPolicy {
    pub required: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageRegionConstraints {
    pub allowed_jurisdictions: Vec<String>,
    pub default_server_template: String,
    pub requires_public_ingress_review: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageResourceDefaults {
    pub min_vcpu: u32,
    pub min_ram_mb: u32,
    pub min_disk_gb: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageSecurityPolicy {
    pub runs_as_root: bool,
    pub requires_read_only_rootfs: bool,
    pub requires_cap_drop_all: bool,
    pub requires_sbom: bool,
    pub requires_provenance: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCatalog {
    pub schema_version: u32,
    pub provider_id: String,
    pub display_name: String,
    pub catalog_kind: String,
    pub source_policy: ProviderCatalogSourcePolicy,
    pub api_sources: Vec<ProviderApiSource>,
    pub image_source_types: Vec<ProviderImageSourceType>,
    pub required_dashboard_dimensions: Vec<String>,
    pub generated_cache: Vec<ProviderGeneratedCacheBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCatalogSourcePolicy {
    pub source_of_truth: String,
    pub cache_is_generated: bool,
    pub cache_must_not_contain_secrets: bool,
    pub max_cache_age_seconds: u32,
    pub operator_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderApiSource {
    #[serde(rename = "id")]
    pub source_id: String,
    pub method: String,
    pub path: String,
    pub required_for: Vec<String>,
    pub required_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderImageSourceType {
    #[serde(rename = "id")]
    pub source_type_id: String,
    pub display_name: String,
    pub terraform_fields: Vec<String>,
    pub metadata_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderGeneratedCacheBucket {
    pub bucket_id: String,
    pub item_count: u32,
    pub items: Vec<ProviderGeneratedCacheItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderGeneratedCacheItem {
    pub item_index: u32,
    pub item_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSignalCatalog {
    pub schema_version: u32,
    pub catalog_id: String,
    pub catalog_kind: String,
    pub status_levels: Vec<HealthSignalStatusLevel>,
    pub collection_policy: HealthSignalCollectionPolicy,
    pub signals: Vec<HealthSignalDefinition>,
    pub dashboard_views: Vec<HealthSignalDashboardView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSignalStatusLevel {
    #[serde(rename = "id")]
    pub level_id: String,
    pub rank: u32,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSignalCollectionPolicy {
    pub mode: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSignalDefinition {
    #[serde(rename = "id")]
    pub signal_id: String,
    pub display_name: String,
    pub resource_kind: String,
    pub source: String,
    pub applies_to_server_types: Vec<String>,
    pub fields: Vec<String>,
    pub healthy_when: Vec<String>,
    #[serde(default)]
    pub degraded_when: Vec<String>,
    #[serde(default)]
    pub critical_when: Vec<String>,
    pub implementation_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSignalDashboardView {
    #[serde(rename = "id")]
    pub view_id: String,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacPolicyCatalog {
    pub schema_version: u32,
    pub catalog_id: String,
    pub catalog_kind: String,
    pub principles: Vec<String>,
    pub roles: Vec<RbacRole>,
    pub approval_rules: Vec<RbacApprovalRule>,
    pub audit_policy: RbacAuditPolicy,
    pub ui_policy: RbacUiPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacRole {
    #[serde(rename = "id")]
    pub role_id: String,
    pub display_name: String,
    pub description: String,
    pub allowed_risks: Vec<String>,
    pub denied_actions: Vec<String>,
    pub requires_incident_id: Option<bool>,
    pub max_session_minutes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacApprovalRule {
    #[serde(rename = "id")]
    pub rule_id: String,
    #[serde(default)]
    pub environment_types: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub action_ids: Vec<String>,
    pub required_role: String,
    pub required_approver_count: u32,
    pub allow_self_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacAuditPolicy {
    pub retention_days: u32,
    pub capture_request: bool,
    pub capture_catalog_version: bool,
    pub capture_actor: bool,
    pub capture_approvers: bool,
    pub capture_preflight_results: bool,
    pub capture_stdout_stderr_redacted: bool,
    pub never_capture_secret_values: bool,
    pub never_capture_unredacted_backup_urls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacUiPolicy {
    pub hide_denied_actions: bool,
    pub show_denied_reason: bool,
    pub require_reauthentication_for_destructive: bool,
    pub require_fresh_health_before_change_seconds: u32,
    pub require_provider_cache_fresh_before_change_seconds: u32,
    pub disable_actions_when_environment_locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerServiceDefinition {
    pub schema_version: u32,
    pub definition_id: String,
    pub environment_id: String,
    pub service_id: String,
    pub service_class: DockerServiceClassKind,
    #[serde(default)]
    pub services: Vec<ProvisioningService>,
    #[serde(default)]
    pub tailscale_tags: Vec<TailnetTagKind>,
    pub image_id: String,
    pub desired_replicas: u32,
    pub region_policy: DockerServiceRegionPolicy,
    pub compute: DockerServiceCompute,
    pub ingress: DockerServiceIngress,
    pub secrets: Vec<DockerServiceSecretRef>,
    pub lifecycle: DockerServiceLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceMeshProjection {
    pub schema_version: u32,
    pub identity_contract: ServiceMeshIdentityContract,
    pub environment_contracts: Vec<ServiceMeshEnvironmentContract>,
    pub desired_service_identities: Vec<ServiceMeshDesiredIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceMeshIdentityContract {
    pub declared_identity_source_action_id: String,
    pub declared_identity_section_name: String,
    pub declared_identity_count_field: String,
    pub declared_identity_record_fields: Vec<String>,
    pub observed_identity_source_action_id: String,
    pub observed_identity_host_key_field: String,
    pub observed_tailscale_tags_field: String,
    pub observed_tailscale_ipv4_field: String,
    pub observed_tailscale0_ipv4_field: String,
    pub drift_matching_key: String,
    pub drift_policy: String,
    pub drift_failure_conditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceMeshEnvironmentContract {
    pub environment_id: String,
    pub environment_type: ProvisioningEnvironment,
    pub server_type: ProvisioningRuntime,
    pub topology_template: ProvisioningLifecycleEngine,
    pub declared_identity_supported: bool,
    pub declared_identity_transport: Option<ServiceMeshDeclaredIdentityTransport>,
    pub observed_identity_supported: bool,
    pub observed_identity_transport: Option<ServiceMeshObservedIdentityTransport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceMeshDeclaredIdentityTransport {
    pub action_id: String,
    pub format: String,
    pub section_name: String,
    pub count_field: String,
    pub record_fields: Vec<String>,
    pub optional_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceMeshObservedIdentityTransport {
    pub action_id: String,
    pub format: String,
    pub field_name: String,
    pub host_key_field: String,
    pub related_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceMeshDesiredIdentity {
    pub source_path: String,
    pub definition_id: String,
    pub environment_id: String,
    pub service_id: String,
    pub service_class: DockerServiceClassKind,
    pub services: Vec<ProvisioningService>,
    pub tailscale_tags: Vec<TailnetTagKind>,
    pub region_policy: DockerServiceRegionPolicy,
    pub desired_replicas: u32,
    pub image_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerServiceRegionPolicy {
    #[serde(alias = "jurisdiction", alias = "region")]
    pub data_region: ProvisioningDataRegion,
    pub allowed_sites: Vec<ProvisioningSite>,
    pub strategy: RegionStrategy,
    pub same_site_transport: NetworkTransport,
    pub cross_site_transport: NetworkTransport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerServiceCompute {
    pub provider: InfrastructureProvider,
    pub server_template: ServerTemplate,
    #[serde(alias = "plan_id")]
    pub plan: ProvisioningInstancePlan,
    #[serde(alias = "image_source_type")]
    pub machine_image_source: ProvisioningMachineImageSource,
    pub os: Option<ProvisioningOs>,
    pub app_id: Option<String>,
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerServiceIngress {
    pub public: Vec<DockerServiceIngressRule>,
    pub private: Vec<DockerServiceIngressRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerServiceIngressRule {
    pub name: String,
    pub port: Option<u32>,
    pub protocol: Option<String>,
    pub source: Option<NetworkSource>,
    pub requires_load_balancer: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerServiceSecretRef {
    pub name: String,
    pub secret_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerServiceLifecycle {
    pub rollout_strategy: RolloutStrategy,
    pub max_unavailable: u32,
    pub requires_health_before_scale_down: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListWorkflowRunsRequest {
    pub environment_id: Option<String>,
    pub kind: Option<WorkflowRunKind>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListWorkflowRunsResponse {
    pub runs: Vec<WorkflowRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetWorkflowRunRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetWorkflowRunResponse {
    pub run: WorkflowRun,
    pub events: Vec<WorkflowRunEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRunEvent {
    pub sequence: u32,
    pub stream: WorkflowRunEventStream,
    pub line: String,
    pub redacted: bool,
    pub emitted_at_unix_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunEventStream {
    Stdout,
    Stderr,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryTarget {
    pub target: ServiceActionTarget,
    pub systemd_unit_name: Option<String>,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryActionKind {
    RestartSystemdUnit,
    RestartDockerContainer,
    RerunServiceOperation,
    RedeployService,
    RestoreBackup,
    FailoverService,
    FailbackService,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerRecoveryRequest {
    pub environment_id: String,
    pub target: RecoveryTarget,
    pub action: RecoveryActionKind,
    pub requested_by: WorkflowActor,
    pub options: Option<ServiceOperationOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerRecoveryResponse {
    pub run: WorkflowRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateNodeRequest {
    pub provisioning_request: ProvisioningRequest,
    pub requested_by: WorkflowActor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateNodeResponse {
    pub plan: ProvisioningPlan,
    pub run: WorkflowRun,
    pub inventory_rows: Vec<HephaestusServerInventoryRow>,
    pub reconciliation_status: ProvisioningReconciliationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteNodeRequest {
    pub server_id: String,
    pub idempotency_key: String,
    pub requested_by: WorkflowActor,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteNodeResponse {
    pub server_id: String,
    pub reconciliation_status: ProvisioningReconciliationStatus,
    pub inventory_row: Option<HephaestusServerInventoryRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPowerActionKind {
    SoftReboot,
    HardReset,
    PowerOn,
    PowerOff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPowerActionRequest {
    pub server_id: String,
    pub action: ProviderPowerActionKind,
    pub idempotency_key: String,
    pub requested_by: WorkflowActor,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPowerActionResponse {
    pub server_id: String,
    pub action: ProviderPowerActionKind,
    pub reconciliation_status: ProvisioningReconciliationStatus,
    pub inventory_row: Option<HephaestusServerInventoryRow>,
    pub requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeNodeRequest {
    pub server_id: String,
    pub target_plan: ProvisioningInstancePlan,
    pub upgrade_disk: bool,
    pub idempotency_key: String,
    pub requested_by: WorkflowActor,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeNodeResponse {
    pub server_id: String,
    pub target_plan: ProvisioningInstancePlan,
    pub reconciliation_status: ProvisioningReconciliationStatus,
    pub inventory_row: Option<HephaestusServerInventoryRow>,
    pub requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateIpv4EgressGatewayRequest {
    pub provider: InfrastructureProvider,
    pub environment: ProvisioningEnvironment,
    pub data_region: ProvisioningDataRegion,
    pub site: ProvisioningSite,
    pub plan: ProvisioningInstancePlan,
    pub private_ipv4: String,
    pub server_id: String,
    pub idempotency_key: String,
    pub requested_by: WorkflowActor,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateIpv4EgressGatewayResponse {
    pub server_id: String,
    pub provider_server_id: String,
    pub private_ipv4: String,
    pub network_zone: String,
    pub reconciliation_status: ProvisioningReconciliationStatus,
    pub inventory_row: Option<HephaestusServerInventoryRow>,
    pub requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFoundationDbReconfigureRequest {
    pub environment: ProvisioningEnvironment,
    pub data_region: ProvisioningDataRegion,
    pub redundancy_mode: FoundationDbRedundancyMode,
    pub idempotency_key: String,
    pub requested_by: WorkflowActor,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFoundationDbReconfigureResponse {
    pub environment: ProvisioningEnvironment,
    pub data_region: ProvisioningDataRegion,
    pub redundancy_mode: FoundationDbRedundancyMode,
    pub target_node_ids: Vec<String>,
    pub reconciliation_status: ProvisioningReconciliationStatus,
    pub requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetNodeRequest {
    pub server_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetNodeResponse {
    pub inventory_row: Option<HephaestusServerInventoryRow>,
    pub actual_state: Option<HephaestusNodeActualState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileNodeRequest {
    pub server_id: String,
    pub idempotency_key: String,
    pub requested_by: WorkflowActor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileNodeResponse {
    pub server_id: String,
    pub reconciliation_status: ProvisioningReconciliationStatus,
    pub inventory_row: Option<HephaestusServerInventoryRow>,
    pub actual_state: Option<HephaestusNodeActualState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetNodeHealthRequest {
    pub server_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentReportStatus {
    Unspecified,
    RegisteredNoReport,
    Reporting,
    Stale,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetNodeHealthResponse {
    pub server_id: String,
    pub reconciliation_status: ProvisioningReconciliationStatus,
    pub inventory_row: Option<HephaestusServerInventoryRow>,
    pub actual_state: Option<HephaestusNodeActualState>,
    pub agent_report_status: AgentReportStatus,
    pub has_full_report: bool,
    pub latest_agent_report: Option<HephaestusAgentReport>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListApprovedNodeChoicesRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListApprovedNodeChoicesResponse {
    pub sites: Vec<DashboardSiteOption>,
    pub plans: Vec<DashboardPlanOption>,
    pub services: Vec<DashboardServiceOption>,
    pub provider_catalogs: Vec<ProviderCatalog>,
    pub location_matrix: ProviderLocationMatrix,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetOperationalOverviewRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetOperationalOverviewResponse {
    pub environments: Vec<InfrastructureEnvironment>,
    pub environment_locks: Vec<EnvironmentApplyLock>,
    pub recent_runs: Vec<WorkflowRun>,
    pub inventory_rows: Vec<HephaestusServerInventoryRow>,
    pub service_health: Vec<ServiceHealthAggregate>,
    pub runbook_catalog: Option<RunbookCatalog>,
    pub docker_image_catalog: Option<DockerImageCatalog>,
    pub provider_catalogs: Vec<ProviderCatalog>,
    pub health_signal_catalog: Option<HealthSignalCatalog>,
    pub rbac_policy: Option<RbacPolicyCatalog>,
    pub docker_service_definitions: Vec<DockerServiceDefinition>,
    pub action_request_examples: Vec<InfrastructureActionRequest>,
    pub json_schemas: Vec<InfrastructureJsonSchemaDocument>,
    pub operational_documents: Vec<InfrastructureOperationalDocument>,
    pub health_snapshots: Vec<InfrastructureHealthSnapshot>,
    pub service_mesh: Option<ServiceMeshProjection>,
    pub internal_dns: Option<InternalDnsProjection>,
    pub observability: Option<ObservabilityProjection>,
    pub node_topology: Option<NodeTopologyProjection>,
    pub dashboard_control_surface: Option<DashboardControlSurfaceProjection>,
    pub tailnet: Option<TailnetOverview>,
    pub control_plane_surfaces: Option<ControlPlaneSurfaceCatalog>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetTailnetOverviewRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetTailnetOverviewResponse {
    pub tailnet: TailnetOverview,
    pub control_plane_surfaces: ControlPlaneSurfaceCatalog,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlPlaneSurfaceCatalog {
    pub schema_version: u32,
    pub opentofu: OpenTofuSurface,
    pub ansible: AnsibleSurface,
    pub tailscale: TailscaleApiSurface,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenTofuSurface {
    pub operations: Vec<OpenTofuOperation>,
    pub resource_kinds: Vec<OpenTofuResourceKind>,
    pub change_actions: Vec<OpenTofuChangeAction>,
    pub change_reasons: Vec<OpenTofuChangeReason>,
    pub sensitive_json_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenTofuOperation {
    Init,
    Validate,
    Plan,
    Apply,
    Destroy,
    ShowPlanJson,
    ShowStateJson,
    OutputJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenTofuResourceKind {
    Server,
    Network,
    Firewall,
    LoadBalancer,
    SshKey,
    Image,
    Snapshot,
    Volume,
    ObjectStorage,
    Dns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenTofuChangeAction {
    Noop,
    Create,
    Read,
    Update,
    Replace,
    Delete,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenTofuChangeReason {
    Requested,
    Tainted,
    CannotUpdate,
    MissingResourceConfig,
    ProviderDrift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnsibleSurface {
    pub operations: Vec<AnsibleOperation>,
    pub event_kinds: Vec<AnsibleEventKind>,
    pub host_statuses: Vec<AnsibleHostStatus>,
    pub supported_runner_artifacts: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnsibleOperation {
    RunPlaybook,
    CheckPlaybook,
    GatherFacts,
    RenderInventory,
    InspectRoleArgumentSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnsibleEventKind {
    PlaybookOnStart,
    PlaybookOnPlayStart,
    PlaybookOnTaskStart,
    RunnerOnOk,
    RunnerOnChanged,
    RunnerOnFailed,
    RunnerOnUnreachable,
    RunnerOnSkipped,
    PlaybookOnStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnsibleHostStatus {
    Ok,
    Changed,
    Failed,
    Unreachable,
    Skipped,
    Rescued,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailscaleApiSurface {
    pub operations: Vec<TailscaleApiOperation>,
    pub policy_sections: Vec<TailscalePolicySection>,
    pub device_operations: Vec<TailscaleDeviceOperation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TailscaleApiOperation {
    ListDevices,
    GetDevice,
    GetPolicy,
    ValidatePolicy,
    ListServices,
    GetService,
    ListRoutes,
    ListAuthKeys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TailscalePolicySection {
    TagOwners,
    Grants,
    Acls,
    Ssh,
    AutoApprovers,
    Groups,
    Hosts,
    NodeAttrs,
    Tests,
    SshTests,
    IpSets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TailscaleDeviceOperation {
    UpdateTags,
    Authorize,
    ExpireKey,
    Delete,
    ApproveRoutes,
    RevokeRoutes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailnetOverview {
    pub schema_version: u32,
    pub tailnet: String,
    pub collected_at: HephaestusTimestamp,
    pub devices: Vec<TailnetDevice>,
    pub tags: Vec<TailnetTag>,
    pub services: Vec<TailnetService>,
    pub policy: Option<TailnetPolicySummary>,
    pub drift_findings: Vec<TailnetDriftFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailnetDevice {
    pub hostname: String,
    pub device_id: Option<String>,
    pub magic_dns_name: Option<String>,
    pub addresses: Vec<String>,
    pub tags: Vec<String>,
    pub online: Option<bool>,
    pub last_seen: Option<String>,
    pub os: Option<String>,
    pub client_version: Option<String>,
    pub advertised_routes: Vec<TailnetRoute>,
    pub approved_routes: Vec<TailnetRoute>,
    pub key_expires: Option<String>,
    pub desired_node_id: Option<String>,
    pub drift_status: TailnetDeviceDriftStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailnetRoute {
    pub cidr: String,
    pub approved: bool,
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TailnetDeviceDriftStatus {
    Unknown,
    InSync,
    Missing,
    TagDrift,
    StaleCollision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailnetTag {
    pub tag: String,
    pub owners: Vec<String>,
    pub device_hostnames: Vec<String>,
    pub declared_in_policy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailnetService {
    pub service_name: String,
    pub magic_dns_names: Vec<String>,
    pub hosts: Vec<TailnetServiceHost>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailnetServiceHost {
    pub hostname: String,
    pub device_id: Option<String>,
    pub advertised: bool,
    pub approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailnetPolicySummary {
    pub source_path: Option<String>,
    pub etag: Option<String>,
    pub sha256: Option<String>,
    pub tag_owner_count: u32,
    pub grant_count: u32,
    pub acl_count: u32,
    pub ssh_rule_count: u32,
    pub auto_approver_count: u32,
    pub uses_grants: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailnetDriftFinding {
    pub severity: TailnetDriftSeverity,
    pub subject: String,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TailnetDriftSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardControlSurfaceProjection {
    pub schema_version: u32,
    pub catalog_id: String,
    pub description: String,
    pub naming_policy: DashboardNamingPolicy,
    pub state_model: DashboardStateModel,
    pub menus: DashboardMenus,
    pub actions: Vec<DashboardAction>,
    pub blocking_followups: Vec<DashboardBlockingFollowup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardNamingPolicy {
    pub non_role_template: String,
    pub role_template: String,
    pub ordinal_policy: String,
    pub operator_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardStateModel {
    pub template_catalog: String,
    pub runtime_instance_state: String,
    pub runtime_instance_state_policy: String,
    pub provider_state: String,
    pub health_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardMenus {
    pub environments: Vec<DashboardEnvironmentOption>,
    pub sites: Vec<DashboardSiteOption>,
    pub plans: Vec<DashboardPlanOption>,
    pub services: Vec<DashboardServiceOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardEnvironmentOption {
    pub id: ProvisioningEnvironment,
    pub label: String,
    pub enabled: bool,
    pub risk: DashboardEnvironmentRisk,
    #[serde(default, rename = "default")]
    pub is_default: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardSiteOption {
    pub id: ProvisioningSite,
    pub label: String,
    pub provider_location: ProvisioningSite,
    pub data_region: ProvisioningDataRegion,
    pub enabled_for: Vec<ProvisioningEnvironment>,
    pub placement_kind: ProvisioningPlacementKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardPlanOption {
    pub id: ProvisioningInstancePlan,
    pub label: String,
    pub enabled_for: Vec<ProvisioningEnvironment>,
    #[serde(default)]
    pub default_for: Vec<ProvisioningService>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardServiceOption {
    pub id: ProvisioningService,
    pub label: String,
    pub enabled: bool,
    pub runtime: ProvisioningRuntime,
    pub roles: Vec<DashboardServiceRoleOption>,
    #[serde(default)]
    pub default_plan: Option<ProvisioningInstancePlan>,
    #[serde(default)]
    pub default_ingress_mode: Option<ProvisioningIngressMode>,
    #[serde(default)]
    pub container_image_menu: Vec<ProvisioningContainerImageSource>,
    pub create_status: DashboardCreateStatus,
    pub health_groups: Vec<ProvisioningHealthGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardServiceRoleOption {
    #[serde(default)]
    pub nats_role: Option<ProvisioningNatsRole>,
    #[serde(default)]
    pub typesense_role: Option<ProvisioningTypesenseRole>,
    #[serde(default)]
    pub foundationdb_role: Option<ProvisioningFoundationDbRole>,
    #[serde(default)]
    pub workload_role: Option<ProvisioningWorkloadRole>,
    pub label: String,
    #[serde(default, rename = "default")]
    pub is_default: bool,
    pub description: Option<String>,
    pub foundationdb: Option<DashboardFoundationDbRoleSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardFoundationDbRoleSettings {
    pub data_bearing: bool,
    pub coordinator_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardAction {
    pub id: ProvisioningLifecycleAction,
    pub label: String,
    pub risk: RunbookRiskLevel,
    pub button: bool,
    pub input_fields: Vec<DashboardInputField>,
    pub argv: Vec<String>,
    pub post_success_recommended_action: Option<ProvisioningLifecycleAction>,
    #[serde(default)]
    pub requires_node_selector: bool,
    #[serde(default)]
    pub forbid_destroy_all_without_extra_confirmation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardBlockingFollowup {
    pub id: String,
    pub priority: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeTopologyProjection {
    pub schema_version: u32,
    pub resolver: NodeTopologyResolverContract,
    pub service_templates: Vec<NodeServiceTemplate>,
    pub nodes: Vec<NodeTopologyNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeTopologyResolverContract {
    pub executable: String,
    pub catalog_path: String,
    pub input_contract: String,
    pub output_contract: String,
    pub command_model: String,
    pub execute_via_shell: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeServiceTemplate {
    pub template_id: String,
    pub base_template_id: Option<String>,
    pub lifecycle_engine: String,
    pub provider_environment_id: String,
    #[serde(alias = "service_roles")]
    pub service_identity_labels: Vec<String>,
    pub tailscale_tags: Vec<String>,
    pub service_dns_label: Option<String>,
    pub service_provider_tag: Option<String>,
    pub default_port: Option<u32>,
    pub default_plan: Option<String>,
    pub implementation_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeTopologyNode {
    pub node_id: String,
    pub environment: String,
    pub service: String,
    pub site: String,
    pub data_region: String,
    pub provider_environment_id: String,
    pub state_identity: String,
    pub ordinal: u32,
    pub hostname: String,
    pub label: String,
    pub plan: String,
    pub foundationdb: Option<NodeFoundationDbRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFoundationDbRole {
    pub coordinator: bool,
    pub data_bearing: bool,
    pub bootstrap: bool,
    pub role: String,
    pub process_class: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentApplyLock {
    pub environment_id: String,
    pub apply_in_progress: bool,
    pub active_run_id: Option<String>,
    pub locked_at: HephaestusTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceHealthAggregate {
    pub environment_id: String,
    pub service_id: String,
    pub service_kind: ServiceKind,
    pub status: ServiceHealthStatus,
    pub checked_at: HephaestusTimestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceHealthStatus {
    Unknown,
    Healthy,
    Degraded,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub schema_version: u32,
    pub snapshot_id: String,
    pub environment_id: String,
    pub collected_at: HephaestusTimestamp,
    pub overall_status: ServiceHealthStatus,
    pub signals: Vec<HealthSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSignal {
    pub signal_id: String,
    pub status: ServiceHealthStatus,
    pub collected_at: HephaestusTimestamp,
    pub source: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfrastructureHealthSnapshot {
    pub schema_version: u32,
    pub snapshot_id: String,
    pub environment_id: String,
    pub collected_at: HephaestusTimestamp,
    pub overall_status: ServiceHealthStatus,
    pub signals: Vec<InfrastructureHealthSignal>,
    #[serde(default)]
    pub tailscale_identities: Vec<TailscaleIdentitySnapshot>,
    pub docker_runtime: Option<DockerRuntimeHealth>,
    pub observability: Option<HostObservabilityHealth>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostObservabilityHealth {
    pub node_exporter: Option<NodeExporterHealth>,
    pub cadvisor: Option<CadvisorHealth>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeExporterHealth {
    pub service_status: String,
    pub metrics_reachable: bool,
    pub port: u32,
    pub scrape_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CadvisorHealth {
    pub container_name: String,
    pub running: bool,
    pub metrics_reachable: bool,
    pub port: u32,
    pub scrape_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailscaleIdentitySnapshot {
    pub hostname: String,
    #[serde(alias = "declared_service_roles")]
    pub declared_service_identity_labels: Vec<String>,
    pub declared_tailscale_tags: Vec<String>,
    pub observed_tailscale_tags: Vec<String>,
    pub site: Option<String>,
    pub jurisdiction: Option<String>,
    pub cluster: Option<String>,
    pub tailscale_ipv4: Option<String>,
    pub tailscale0_ipv4: Option<String>,
    pub drift_status: TailscaleIdentityDriftStatus,
    pub missing_declared_tags: Vec<String>,
    pub unexpected_observed_tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TailscaleIdentityDriftStatus {
    Unknown,
    InSync,
    Drifted,
    Unobserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerRuntimeHealth {
    pub docker_status: String,
    pub containerd_status: String,
    pub docker_server_version: Option<String>,
    pub docker_storage_driver: Option<String>,
    pub docker_cgroup_driver: Option<String>,
    pub docker_containers_running: u32,
    pub docker_images_count: u32,
    pub docker_container_count: u32,
    pub containers: Vec<DockerContainerHealth>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerContainerHealth {
    pub index: u32,
    pub container_id: Option<String>,
    pub name: String,
    pub status: String,
    pub running: bool,
    pub health: Option<String>,
    pub restart_count: u32,
    pub started_at: Option<HephaestusTimestamp>,
    pub image_ref: Option<String>,
    pub image_id: Option<String>,
    pub image_architecture: Option<String>,
    pub image_digest: Option<String>,
    pub command: Option<String>,
    pub port_bindings: Vec<String>,
    pub mounts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfrastructureHealthSignal {
    pub signal_id: String,
    pub status: ServiceHealthStatus,
    pub collected_at: HephaestusTimestamp,
    pub source: String,
    pub summary: String,
    pub facts: Vec<InfrastructureHealthFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfrastructureHealthFact {
    pub key: String,
    pub value_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitAgentReportRequest {
    report: HephaestusAgentReport,
}

impl SubmitAgentReportRequest {
    /// Constructs an agent-report submission request.
    pub const fn new(report: HephaestusAgentReport) -> Self {
        Self { report }
    }

    /// Returns the submitted report payload.
    pub const fn report(&self) -> &HephaestusAgentReport {
        &self.report
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitAgentReportResponse {
    report: HephaestusAgentReport,
}

impl SubmitAgentReportResponse {
    /// Constructs an agent-report submission response.
    pub const fn new(report: HephaestusAgentReport) -> Self {
        Self { report }
    }

    /// Returns the accepted report payload.
    pub const fn report(&self) -> &HephaestusAgentReport {
        &self.report
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetAgentReportByServerIdRequest {
    server_id: HephaestusServerId,
}

impl GetAgentReportByServerIdRequest {
    /// Constructs an agent-report lookup request.
    pub const fn new(server_id: HephaestusServerId) -> Self {
        Self { server_id }
    }

    /// Returns the internal server identifier.
    pub const fn server_id(&self) -> &HephaestusServerId {
        &self.server_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetAgentReportByServerIdResponse {
    report: Option<HephaestusAgentReport>,
}

impl GetAgentReportByServerIdResponse {
    /// Constructs an agent-report lookup response.
    pub const fn new(report: Option<HephaestusAgentReport>) -> Self {
        Self { report }
    }

    /// Returns the latest report when one is known.
    pub const fn report(&self) -> Option<&HephaestusAgentReport> {
        self.report.as_ref()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListAgentReportsRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListAgentReportsResponse {
    reports: Vec<HephaestusAgentReport>,
}

impl ListAgentReportsResponse {
    /// Constructs an agent-report listing response.
    pub fn new(reports: Vec<HephaestusAgentReport>) -> Self {
        Self { reports }
    }

    /// Returns the latest reports.
    pub fn reports(&self) -> &[HephaestusAgentReport] {
        self.reports.as_slice()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetNodeLiveReportByServerIdRequest {
    server_id: HephaestusServerId,
}

impl GetNodeLiveReportByServerIdRequest {
    /// Constructs a live-report lookup request.
    pub const fn new(server_id: HephaestusServerId) -> Self {
        Self { server_id }
    }

    /// Returns the internal server identity.
    pub const fn server_id(&self) -> &HephaestusServerId {
        &self.server_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetNodeLiveReportByServerIdResponse {
    report: HephaestusAgentReport,
}

impl GetNodeLiveReportByServerIdResponse {
    /// Constructs a live-report response.
    pub const fn new(report: HephaestusAgentReport) -> Self {
        Self { report }
    }

    /// Returns the fetched live report.
    pub const fn report(&self) -> &HephaestusAgentReport {
        &self.report
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentActionCompletionStatus {
    Succeeded,
    Failed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentActionCompletionReason {
    Ok,
    InvalidAction,
    Expired,
    WrongNode,
    LocalCommandFailed,
    LocalValidationFailed,
    FoundationDbServiceStartFailed,
    FoundationDbConfigureNewFailed,
    FoundationDbStatusUnavailable,
    FoundationDbCoordinatorsUnavailable,
    FoundationDbDatabaseNotConfigured,
    FoundationDbDatabaseUnavailable,
    FoundationDbConfigurationInvalid,
    FoundationDbStaleClusterFile,
    FoundationDbRecruitmentPending,
    FoundationDbRecoveryInProgress,
    FoundationDbDataMovementInProgress,
    FoundationDbCoordinatorQuorumLost,
    FoundationDbInsufficientStorageForRedundancy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueAgentActionRequest {
    pub node_id: HephaestusServerId,
    pub action_id: String,
    pub idempotency_key: String,
    pub action_json: String,
    pub created_at_unix_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueAgentActionResponse {
    pub action_json: String,
    pub queued: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListQueuedAgentActionsRequest {
    pub node_id: HephaestusServerId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListQueuedAgentActionsResponse {
    pub action_json_values: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentActionQueueStatus {
    Queued,
    Delivered,
    Succeeded,
    Failed,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentActionRecord {
    pub action_json: String,
    pub queue_status: AgentActionQueueStatus,
    pub created_at_unix_secs: u64,
    pub delivered_at_unix_secs: Option<u64>,
    pub completed_at_unix_secs: Option<u64>,
    pub completion_status: Option<AgentActionCompletionStatus>,
    pub completion_reason: Option<AgentActionCompletionReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListAgentActionRecordsRequest {
    pub node_id: HephaestusServerId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListAgentActionRecordsResponse {
    pub records: Vec<AgentActionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollQueuedAgentActionsRequest {
    pub node_id: HephaestusServerId,
    pub now_unix_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollQueuedAgentActionsResponse {
    pub action_json_values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompleteQueuedAgentActionRequest {
    pub node_id: HephaestusServerId,
    pub action_id: String,
    pub idempotency_key: String,
    pub status: AgentActionCompletionStatus,
    pub reason: AgentActionCompletionReason,
    pub completed_at_unix_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompleteQueuedAgentActionResponse {
    pub accepted: bool,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        InfrastructureProvider, PROVIDER_LOCATION_MATRIX, ProvisioningDataRegion,
        ProvisioningPlacementKind, ProvisioningSite, provider_location_mapping,
    };

    #[test]
    fn provider_location_matrix_maps_provider_specific_ids_to_reallyme_sites() {
        assert_eq!(
            provider_location_mapping(InfrastructureProvider::Vultr, "sgp").map(|mapping| {
                (
                    mapping.site,
                    mapping.data_region,
                    mapping.placement_kind,
                    mapping.canonical,
                )
            }),
            Some((
                ProvisioningSite::Sin,
                ProvisioningDataRegion::None,
                ProvisioningPlacementKind::Edge,
                false
            ))
        );
        assert_eq!(
            provider_location_mapping(InfrastructureProvider::Vultr, "sao").map(|mapping| {
                (
                    mapping.site,
                    mapping.data_region,
                    mapping.placement_kind,
                    mapping.canonical,
                )
            }),
            Some((
                ProvisioningSite::Gru,
                ProvisioningDataRegion::None,
                ProvisioningPlacementKind::Edge,
                false
            ))
        );
        assert_eq!(
            provider_location_mapping(InfrastructureProvider::Hetzner, "fsn1").map(|mapping| {
                (
                    mapping.site,
                    mapping.data_region,
                    mapping.placement_kind,
                    mapping.canonical,
                )
            }),
            Some((
                ProvisioningSite::Lej,
                ProvisioningDataRegion::Eu,
                ProvisioningPlacementKind::Regional,
                false
            ))
        );
        assert_eq!(
            provider_location_mapping(InfrastructureProvider::Hetzner, "ash").map(|mapping| {
                (
                    mapping.site,
                    mapping.data_region,
                    mapping.placement_kind,
                    mapping.canonical,
                )
            }),
            Some((
                ProvisioningSite::Iad,
                ProvisioningDataRegion::Us,
                ProvisioningPlacementKind::Regional,
                false
            ))
        );
    }

    #[test]
    fn provider_location_matrix_has_unique_provider_location_keys() {
        let mut seen = BTreeSet::new();
        for mapping in PROVIDER_LOCATION_MATRIX {
            assert!(
                seen.insert((
                    mapping.provider.as_catalog_str(),
                    mapping.provider_location_id
                )),
                "duplicate provider location key"
            );
        }
    }

    #[test]
    fn provider_location_matrix_rejects_unknown_provider_location() {
        assert_eq!(
            provider_location_mapping(InfrastructureProvider::Hetzner, "ams"),
            None
        );
        assert_eq!(
            provider_location_mapping(InfrastructureProvider::Vultr, "fsn1"),
            None
        );
    }
}
