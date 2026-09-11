// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(missing_docs)]

use std::future::Future;
use std::pin::Pin;

use reallyme_app_kit::{
    AppDownstreamPortDescriptor, AppPortDescriptor, AppPortError, AppPortErrorKind, AppPortName,
};

use crate::{
    ApplyInfraRequest, ApplyInfraResponse, CompleteQueuedAgentActionRequest,
    CompleteQueuedAgentActionResponse, DeleteNodeRequest, DeleteNodeResponse,
    GetAgentReportByServerIdRequest, GetAgentReportByServerIdResponse, GetDesiredTopologyRequest,
    GetDesiredTopologyResponse, GetNodeActualStateByServerIdRequest,
    GetNodeActualStateByServerIdResponse, GetNodeHealthRequest, GetNodeHealthResponse,
    GetNodeLiveReportByServerIdRequest, GetNodeLiveReportByServerIdResponse, GetNodeRequest,
    GetNodeResponse, GetOperationalOverviewRequest, GetOperationalOverviewResponse,
    GetTailnetOverviewRequest, GetTailnetOverviewResponse, GetWorkflowRunRequest,
    GetWorkflowRunResponse, ListAgentActionRecordsRequest, ListAgentActionRecordsResponse,
    ListAgentReportsRequest, ListAgentReportsResponse, ListApprovedNodeChoicesRequest,
    ListApprovedNodeChoicesResponse, ListNodeActualStatesRequest, ListNodeActualStatesResponse,
    ListQueuedAgentActionsRequest, ListQueuedAgentActionsResponse, ListServerInventoryRequest,
    ListServerInventoryResponse, ListWorkflowRunsRequest, ListWorkflowRunsResponse,
    PlanInfraRequest, PlanInfraResponse, PollQueuedAgentActionsRequest,
    PollQueuedAgentActionsResponse, ProviderPowerActionRequest, ProviderPowerActionResponse,
    QueueAgentActionRequest, QueueAgentActionResponse, ReconcileNodeRequest, ReconcileNodeResponse,
    ResizeNodeRequest, ResizeNodeResponse, RunAnsibleRequest, RunAnsibleResponse,
    RunHealthChecksRequest, RunHealthChecksResponse, RunRunbookActionRequest,
    RunRunbookActionResponse, SubmitAgentBootReportRequest, SubmitAgentBootReportResponse,
    SubmitAgentReportRequest, SubmitAgentReportResponse, TriggerRecoveryRequest,
    TriggerRecoveryResponse, UpdateDesiredTopologyRequest, UpdateDesiredTopologyResponse,
};

pub type HephaestusPortErrorKind = AppPortErrorKind;

pub type HephaestusPortError = AppPortError;

pub type HephaestusPortCall<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, HephaestusPortError>> + Send + 'a>>;

pub trait TopologyRepositoryPort: Send + Sync {
    fn validate_startup(&self) -> Result<(), HephaestusPortError>;

    fn get_desired_topology(
        &self,
        request: GetDesiredTopologyRequest,
    ) -> HephaestusPortCall<'_, GetDesiredTopologyResponse>;

    fn update_desired_topology(
        &self,
        request: UpdateDesiredTopologyRequest,
    ) -> HephaestusPortCall<'_, UpdateDesiredTopologyResponse>;
}

pub trait WorkflowEnginePort: Send + Sync {
    fn validate_startup(&self) -> Result<(), HephaestusPortError>;

    fn plan_infra(&self, request: PlanInfraRequest) -> HephaestusPortCall<'_, PlanInfraResponse>;

    fn apply_infra(&self, request: ApplyInfraRequest)
    -> HephaestusPortCall<'_, ApplyInfraResponse>;

    fn run_ansible(&self, request: RunAnsibleRequest)
    -> HephaestusPortCall<'_, RunAnsibleResponse>;

    fn run_health_checks(
        &self,
        request: RunHealthChecksRequest,
    ) -> HephaestusPortCall<'_, RunHealthChecksResponse>;

    fn run_runbook_action(
        &self,
        request: RunRunbookActionRequest,
    ) -> HephaestusPortCall<'_, RunRunbookActionResponse>;

    fn trigger_recovery(
        &self,
        request: TriggerRecoveryRequest,
    ) -> HephaestusPortCall<'_, TriggerRecoveryResponse>;
}

pub trait OperationalStatePort: Send + Sync {
    fn validate_startup(&self) -> Result<(), HephaestusPortError>;

    fn list_workflow_runs(
        &self,
        request: ListWorkflowRunsRequest,
    ) -> HephaestusPortCall<'_, ListWorkflowRunsResponse>;

    fn get_workflow_run(
        &self,
        request: GetWorkflowRunRequest,
    ) -> HephaestusPortCall<'_, GetWorkflowRunResponse>;

    fn get_operational_overview(
        &self,
        request: GetOperationalOverviewRequest,
    ) -> HephaestusPortCall<'_, GetOperationalOverviewResponse>;

    fn get_tailnet_overview(
        &self,
        request: GetTailnetOverviewRequest,
    ) -> HephaestusPortCall<'_, GetTailnetOverviewResponse>;

    fn submit_report(
        &self,
        request: SubmitAgentReportRequest,
    ) -> HephaestusPortCall<'_, SubmitAgentReportResponse>;

    fn list_reports(
        &self,
        request: ListAgentReportsRequest,
    ) -> HephaestusPortCall<'_, ListAgentReportsResponse>;

    fn get_report_by_server_id(
        &self,
        request: GetAgentReportByServerIdRequest,
    ) -> HephaestusPortCall<'_, GetAgentReportByServerIdResponse>;

    fn submit_boot_report(
        &self,
        request: SubmitAgentBootReportRequest,
    ) -> HephaestusPortCall<'_, SubmitAgentBootReportResponse>;

    fn list_node_actual_states(
        &self,
        request: ListNodeActualStatesRequest,
    ) -> HephaestusPortCall<'_, ListNodeActualStatesResponse>;

    fn get_node_actual_state_by_server_id(
        &self,
        request: GetNodeActualStateByServerIdRequest,
    ) -> HephaestusPortCall<'_, GetNodeActualStateByServerIdResponse>;

    fn list_server_inventory(
        &self,
        request: ListServerInventoryRequest,
    ) -> HephaestusPortCall<'_, ListServerInventoryResponse>;

    fn get_node(&self, request: GetNodeRequest) -> HephaestusPortCall<'_, GetNodeResponse>;

    fn delete_node(&self, request: DeleteNodeRequest)
    -> HephaestusPortCall<'_, DeleteNodeResponse>;

    fn request_provider_power_action(
        &self,
        request: ProviderPowerActionRequest,
    ) -> HephaestusPortCall<'_, ProviderPowerActionResponse>;

    fn resize_node(&self, request: ResizeNodeRequest)
    -> HephaestusPortCall<'_, ResizeNodeResponse>;

    fn reconcile_node(
        &self,
        request: ReconcileNodeRequest,
    ) -> HephaestusPortCall<'_, ReconcileNodeResponse>;

    fn get_node_health(
        &self,
        request: GetNodeHealthRequest,
    ) -> HephaestusPortCall<'_, GetNodeHealthResponse>;

    fn list_approved_node_choices(
        &self,
        request: ListApprovedNodeChoicesRequest,
    ) -> HephaestusPortCall<'_, ListApprovedNodeChoicesResponse>;

    fn queue_agent_action(
        &self,
        request: QueueAgentActionRequest,
    ) -> HephaestusPortCall<'_, QueueAgentActionResponse>;

    fn list_queued_agent_actions(
        &self,
        request: ListQueuedAgentActionsRequest,
    ) -> HephaestusPortCall<'_, ListQueuedAgentActionsResponse>;

    fn list_agent_action_records(
        &self,
        request: ListAgentActionRecordsRequest,
    ) -> HephaestusPortCall<'_, ListAgentActionRecordsResponse>;

    fn poll_queued_agent_actions(
        &self,
        request: PollQueuedAgentActionsRequest,
    ) -> HephaestusPortCall<'_, PollQueuedAgentActionsResponse>;

    fn complete_queued_agent_action(
        &self,
        request: CompleteQueuedAgentActionRequest,
    ) -> HephaestusPortCall<'_, CompleteQueuedAgentActionResponse>;
}

pub trait NodeAgentControlPort: Send + Sync {
    fn validate_startup(&self) -> Result<(), HephaestusPortError>;

    fn get_live_report(
        &self,
        request: GetNodeLiveReportByServerIdRequest,
    ) -> HephaestusPortCall<'_, GetNodeLiveReportByServerIdResponse>;
}

pub fn topology_repository_port_descriptor()
-> Result<AppDownstreamPortDescriptor, reallyme_app_kit::AppKitError> {
    Ok(AppDownstreamPortDescriptor::new(AppPortDescriptor::new(
        AppPortName::new("topology_repository")?,
        true,
    )))
}

pub fn workflow_engine_port_descriptor()
-> Result<AppDownstreamPortDescriptor, reallyme_app_kit::AppKitError> {
    Ok(AppDownstreamPortDescriptor::new(AppPortDescriptor::new(
        AppPortName::new("workflow_engine")?,
        true,
    )))
}

pub fn operational_state_port_descriptor()
-> Result<AppDownstreamPortDescriptor, reallyme_app_kit::AppKitError> {
    Ok(AppDownstreamPortDescriptor::new(AppPortDescriptor::new(
        AppPortName::new("operational_state")?,
        true,
    )))
}

pub fn node_agent_control_port_descriptor()
-> Result<AppDownstreamPortDescriptor, reallyme_app_kit::AppKitError> {
    Ok(AppDownstreamPortDescriptor::new(AppPortDescriptor::new(
        AppPortName::new("node_agent_control")?,
        false,
    )))
}
