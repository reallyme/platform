// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! Contract crate for the Hephaestus topology-driven infrastructure control plane.

/// Conversion between generated wire DTOs and Hephaestus domain types.
#[cfg(feature = "generated")]
pub mod domain_conversion;
/// Hephaestus app host-neutral DTOs.
pub mod dto;
/// Hephaestus app typed contract errors.
pub mod error;
/// Generated protobuf/Connect boundary.
pub mod generated;
/// Hephaestus node boot-report and actual-state DTOs.
pub mod node;
/// Hephaestus app downstream port traits.
pub mod port;
/// Feature-neutral Hephaestus app RPC metadata.
pub mod rpc;

#[cfg(feature = "generated")]
pub use domain_conversion::{
    agent_boot_report_from_proto, agent_report_from_proto, dns_desired_state_from_proto,
    node_actual_state_from_proto, proto_agent_boot_report_from_domain,
    proto_agent_boot_report_result_from_domain, proto_agent_report_from_domain,
    proto_dns_desired_state_from_domain, proto_node_actual_state_from_domain,
    proto_provisioning_event_snapshot_from_domain, proto_region_definition_from_domain,
    proto_server_inventory_row_from_domain, proto_vultr_container_artifact_from_domain,
    proto_vultr_container_registry_from_domain, proto_vultr_container_repository_from_domain,
    proto_vultr_instance_from_domain, proto_vultr_instance_status_from_domain,
    proto_vultr_instance_template_from_domain, proto_vultr_plan_from_domain,
    proto_vultr_plan_type_from_domain, proto_vultr_region_from_domain, proto_vultr_vpc_from_domain,
    region_definition_from_proto, vultr_container_artifact_from_proto,
    vultr_container_registry_from_proto, vultr_container_repository_from_proto,
    vultr_instance_from_proto, vultr_instance_status_from_proto_i32,
    vultr_instance_template_from_proto, vultr_plan_from_proto, vultr_plan_type_from_proto_i32,
    vultr_region_from_proto, vultr_vpc_from_proto,
};

pub use dto::{
    AgentActionCompletionReason, AgentActionCompletionStatus, AgentActionQueueStatus,
    AgentActionRecord, AgentReportStatus, AnsibleEventKind, AnsibleHostStatus, AnsibleOperation,
    AnsibleSurface, ApplicationRuntimeKind, ApplicationServiceSpec, ApplyInfraRequest,
    ApplyInfraResponse, CadvisorHealth, CompleteQueuedAgentActionRequest,
    CompleteQueuedAgentActionResponse, ContainerImageRef, ControlPlaneSurfaceCatalog,
    CreateIpv4EgressGatewayRequest, CreateIpv4EgressGatewayResponse, CreateNodeRequest,
    CreateNodeResponse, DashboardAction, DashboardBlockingFollowup,
    DashboardControlSurfaceProjection, DashboardCreateStatus, DashboardEnvironmentOption,
    DashboardEnvironmentRisk, DashboardFoundationDbRoleSettings, DashboardInputField,
    DashboardMenus, DashboardNamingPolicy, DashboardPlanOption, DashboardServiceOption,
    DashboardServiceRoleOption, DashboardSiteOption, DashboardStateModel, DeleteNodeRequest,
    DeleteNodeResponse, DependencyKind, DeployableService, DeployableServiceAction,
    DeployableServiceCategory, DeployableServiceImage, DeployableServiceSecret,
    DeployableServiceSummary, DeployableWorkloadKind, DesiredTopologyDocument,
    DesiredTopologyDocumentBody, DesiredTopologySnapshot, DockerContainerHealth,
    DockerDeploymentIntent, DockerDeploymentPlan, DockerEnvironmentBinding,
    DockerHealthProbeIntent, DockerHealthProbeKind, DockerImageApp, DockerImageBackupPolicy,
    DockerImageBuildMetadata, DockerImageCatalog, DockerImageHealthCheck,
    DockerImageMetadataSchema, DockerImageObservability, DockerImagePlan, DockerImagePort,
    DockerImageRegionConstraints, DockerImageRegistry, DockerImageResourceDefaults,
    DockerImageSecret, DockerImageSecurityPolicy, DockerImageSourcePolicy,
    DockerImageSupportedPlatform, DockerImageVolume, DockerNetworkMode, DockerPlannedOperation,
    DockerPortIntent, DockerPortProtocol, DockerRestartPolicy, DockerRuntimeConfig,
    DockerRuntimeHealth, DockerSecretMountIntent, DockerServiceClass, DockerServiceClassKind,
    DockerServiceCompute, DockerServiceDefinition, DockerServiceImage, DockerServiceIngress,
    DockerServiceIngressRule, DockerServiceLifecycle, DockerServiceRegionPolicy,
    DockerServiceSecretRef, DockerValidationError, DockerValidationErrorReason, DockerVolumeIntent,
    EnvironmentApplyLock, EnvironmentTopologyDocument, FailoverMode, FailoverPolicy,
    FoundationDbBackupIntent, FoundationDbBackupPlan, FoundationDbClusterIntent,
    FoundationDbCoordinator, FoundationDbDeploymentPlan, FoundationDbDiskEncryptionMode,
    FoundationDbLifecycleOperation, FoundationDbNodePlan, FoundationDbPlannerRestoreSelector,
    FoundationDbProcessClass, FoundationDbRedundancyMode, FoundationDbRestoreBackupOptions,
    FoundationDbRestoreIntent, FoundationDbRestoreMode, FoundationDbRestorePlan,
    FoundationDbRestoreSelector, FoundationDbRuntimeConfig, FoundationDbServiceSpec,
    FoundationDbSiteBinding, FoundationDbSiteIntent, FoundationDbSiteRole,
    FoundationDbStorageEngine, FoundationDbValidationError, FoundationDbValidationErrorReason,
    GetAgentReportByServerIdRequest, GetAgentReportByServerIdResponse,
    GetDeployableServiceImageRequest, GetDeployableServiceImageResponse,
    GetDeployableServiceRequest, GetDeployableServiceResponse, GetDesiredTopologyRequest,
    GetDesiredTopologyResponse, GetNodeHealthRequest, GetNodeHealthResponse,
    GetNodeLiveReportByServerIdRequest, GetNodeLiveReportByServerIdResponse, GetNodeRequest,
    GetNodeResponse, GetOperationalOverviewRequest, GetOperationalOverviewResponse,
    GetTailnetOverviewRequest, GetTailnetOverviewResponse, GetWorkflowRunRequest,
    GetWorkflowRunResponse, GlobalNetworkingTopology, GlobalTopologyDocument, HealthCheckKind,
    HealthCheckTarget, HealthSignal, HealthSignalCatalog, HealthSignalCollectionPolicy,
    HealthSignalDashboardView, HealthSignalDefinition, HealthSignalStatusLevel, HealthSnapshot,
    HephaestusStatusRequest, HephaestusStatusResponse, HephaestusTimestamp,
    HostObservabilityHealth, InfrastructureActionConfirmation, InfrastructureActionEvent,
    InfrastructureActionEventType, InfrastructureActionRequest, InfrastructureEnvironment,
    InfrastructureHealthFact, InfrastructureHealthSignal, InfrastructureHealthSnapshot,
    InfrastructureJsonSchemaDocument, InfrastructureOperationalDocument, InfrastructureProvider,
    InternalDnsGeneratedOutputContract, InternalDnsNamingPolicy, InternalDnsPolicy,
    InternalDnsProjection, InternalDnsTopologyContract, JetstreamDomainDefinition,
    ListAgentActionRecordsRequest, ListAgentActionRecordsResponse, ListAgentReportsRequest,
    ListAgentReportsResponse, ListApprovedNodeChoicesRequest, ListApprovedNodeChoicesResponse,
    ListDeployableServiceImagesRequest, ListDeployableServiceImagesResponse,
    ListDeployableServicesRequest, ListDeployableServicesResponse, ListQueuedAgentActionsRequest,
    ListQueuedAgentActionsResponse, ListWorkflowRunsRequest, ListWorkflowRunsResponse,
    MachineImageRef, NatsClusterDefinition, NatsDeploymentPlan, NatsGatewayRemote, NatsHttpProbe,
    NatsNodeDeploymentIntent, NatsPortSet, NatsProbeKind, NatsRouteScheme, NatsRouteTarget,
    NatsRuntimeConfig, NatsSuperclusterServiceSpec, NatsValidationError, NatsValidationErrorReason,
    NetworkSource, NetworkTransport, NetworkTransportPolicy, NodeExporterHealth,
    NodeFoundationDbRole, NodeServiceTemplate, NodeTopologyNode, NodeTopologyProjection,
    NodeTopologyResolverContract, ObservabilityAgentContract, ObservabilityDeploymentIntent,
    ObservabilityDeploymentPlan, ObservabilityEnforcementLayer, ObservabilityEnforcementLayers,
    ObservabilityFieldContract, ObservabilityHostAgentKind, ObservabilityHostAgentPlan,
    ObservabilityHostAgents, ObservabilityHttpProbe, ObservabilityProjection,
    ObservabilityRuntimeConfig, ObservabilityScrapePeer, ObservabilitySourceCatalogs,
    ObservabilityValidationError, ObservabilityValidationErrorReason, OpenTofuChangeAction,
    OpenTofuChangeReason, OpenTofuOperation, OpenTofuResourceKind, OpenTofuSurface,
    PROVIDER_LOCATION_MATRIX, PlanDockerDeploymentRequest, PlanDockerDeploymentResponse,
    PlanFoundationDbDeploymentRequest, PlanFoundationDbDeploymentResponse, PlanInfraRequest,
    PlanInfraResponse, PlanNatsDeploymentRequest, PlanNatsDeploymentResponse,
    PlanObservabilityDeploymentRequest, PlanObservabilityDeploymentResponse,
    PlanTypesenseDeploymentRequest, PlanTypesenseDeploymentResponse, PlanVpsRequest,
    PlanVpsResponse, PollQueuedAgentActionsRequest, PollQueuedAgentActionsResponse,
    PrometheusScrapeJob, PrometheusScrapeJobPlan, PrometheusScrapeTargetGenerator,
    PrometheusStaticTarget, ProviderApiSource, ProviderCatalog, ProviderCatalogSourcePolicy,
    ProviderGeneratedCacheBucket, ProviderGeneratedCacheItem, ProviderImageSourceType,
    ProviderLocationCapability, ProviderLocationMapping, ProviderLocationMatrix,
    ProviderPowerActionKind, ProviderPowerActionRequest, ProviderPowerActionResponse,
    ProviderServerCapability, ProviderSiteTokenPolicy, ProvisioningAction, ProvisioningCommand,
    ProvisioningCommandEnvBinding, ProvisioningContainerImageSource, ProvisioningDataRegion,
    ProvisioningEnvironment, ProvisioningExecutionMode, ProvisioningFoundationDbRole,
    ProvisioningHealthGroup, ProvisioningIngressMode, ProvisioningInstancePlan,
    ProvisioningLifecycleAction, ProvisioningLifecycleEngine, ProvisioningMachineImageSource,
    ProvisioningNatsRole, ProvisioningNodeType, ProvisioningOs, ProvisioningPlacementKind,
    ProvisioningPlan, ProvisioningPlanDerived, ProvisioningPlanExecution,
    ProvisioningReconciliationStatus, ProvisioningRequest, ProvisioningRequiredSecretBinding,
    ProvisioningResolutionStatus, ProvisioningRuntime, ProvisioningSecretRef, ProvisioningService,
    ProvisioningSite, ProvisioningSourceRepository, ProvisioningTypesenseRole,
    ProvisioningWorkloadRole, QueueAgentActionRequest, QueueAgentActionResponse, RbacApprovalRule,
    RbacAuditPolicy, RbacPolicyCatalog, RbacRole, RbacUiPolicy, ReconcileNodeRequest,
    ReconcileNodeResponse, RecoveryActionKind, RecoveryTarget, RegionDefinition, RegionPolicy,
    RegionStrategy, RegionTopologyDocument, ReplicationMode, ReplicationPolicy,
    RequestFoundationDbReconfigureRequest, RequestFoundationDbReconfigureResponse,
    ResizeNodeRequest, ResizeNodeResponse, RestoreExecutionMode, RolloutPhase, RolloutPolicy,
    RolloutStrategy, RunAnsibleRequest, RunAnsibleResponse, RunHealthChecksRequest,
    RunHealthChecksResponse, RunRunbookActionRequest, RunRunbookActionResponse,
    RunbookActionApproval, RunbookActionDefinition, RunbookCatalog, RunbookCatalogDefaults,
    RunbookCatalogReference, RunbookCommandSpec, RunbookConfirmationStyle,
    RunbookDashboardContract, RunbookDatabasePolicy, RunbookEnvironmentCommand,
    RunbookEnvironmentDefinition, RunbookEnvironmentRequiredFiles, RunbookExecutionPolicy,
    RunbookImageSelectionPolicy, RunbookIngressRule, RunbookMonitoringSignal,
    RunbookOperatorConfirmation, RunbookPreflightCheckDefinition, RunbookProtocolIngressRule,
    RunbookRegionPolicy, RunbookResourceTemplate, RunbookRiskDefinition, RunbookRiskLevel,
    RunbookScalingPolicy, RunbookServerImageSelectionPolicy, RunbookServerTemplate,
    RunbookServerType, RunbookTopologyServerGroup, RunbookTopologySiteRole,
    RunbookTopologyTemplate, ServerTemplate, ServiceActionTarget, ServiceDefinition,
    ServiceDependency, ServiceGroupDefinition, ServiceHealthAggregate, ServiceHealthStatus,
    ServiceKind, ServiceMeshDeclaredIdentityTransport, ServiceMeshDesiredIdentity,
    ServiceMeshEnvironmentContract, ServiceMeshIdentityContract,
    ServiceMeshObservedIdentityTransport, ServiceMeshProjection, ServiceOperationOptions,
    ServicePort, ServiceSpec, ServiceSubnetPlanEntry, ServicesTopologyDocument, SiteDefinition,
    StaticProviderLocationMapping, SubmitAgentReportRequest, SubmitAgentReportResponse,
    TailnetDevice, TailnetDeviceDriftStatus, TailnetDriftFinding, TailnetDriftSeverity,
    TailnetOverview, TailnetPolicySummary, TailnetRoute, TailnetService, TailnetServiceHost,
    TailnetTag, TailnetTagKind, TailscaleApiOperation, TailscaleApiSurface,
    TailscaleDeviceOperation, TailscaleIdentityDriftStatus, TailscaleIdentitySnapshot,
    TailscalePolicySection, TenantAffinityKind, TenantDefinition, TenantRegionRule,
    TenantRegionTarget, TriggerRecoveryRequest, TriggerRecoveryResponse, TypesenseClusterPeer,
    TypesenseDeploymentPlan, TypesenseHttpProbe, TypesenseLoggingConfig,
    TypesenseNodeDeploymentIntent, TypesenseReplicationGroup, TypesenseResourceGuardrails,
    TypesenseRuntimeConfig, TypesenseServiceSpec, TypesenseTlsConfig, TypesenseValidationError,
    TypesenseValidationErrorReason, UpdateDesiredTopologyRequest, UpdateDesiredTopologyResponse,
    VpsBootstrapPlan, VpsDeploymentIntent, VpsDeploymentPlan, VpsFirewallPlan, VpsPlannedOperation,
    VpsProviderAdapterPlan, VpsRuntimeConfig, VpsValidationError, VpsValidationErrorReason,
    WorkflowActor, WorkflowArtifact, WorkflowArtifactKind, WorkflowOperationKind, WorkflowRun,
    WorkflowRunEvent, WorkflowRunEventStream, WorkflowRunKind, WorkflowRunStatus,
    provider_location_mapping, provider_location_matrix,
};
pub use node::{
    GetNodeActualStateByServerIdRequest, GetNodeActualStateByServerIdResponse,
    ListNodeActualStatesRequest, ListNodeActualStatesResponse, ListServerInventoryRequest,
    ListServerInventoryResponse, RegisterAgentRequest, RegisterAgentResponse,
    SubmitAgentBootReportRequest, SubmitAgentBootReportResponse,
    agent_registration_signature_payload,
};
pub use port::{
    HephaestusPortCall, HephaestusPortError, HephaestusPortErrorKind, NodeAgentControlPort,
    OperationalStatePort, TopologyRepositoryPort, WorkflowEnginePort,
    node_agent_control_port_descriptor, operational_state_port_descriptor,
    topology_repository_port_descriptor, workflow_engine_port_descriptor,
};
pub use rpc::{
    DOCKER_RPC_SERVICE_NAME, HEPHAESTUS_RPC_SERVICE_NAME, HephaestusRpcMethodPath,
    NATS_RPC_SERVICE_NAME, TYPESENSE_RPC_SERVICE_NAME,
};
