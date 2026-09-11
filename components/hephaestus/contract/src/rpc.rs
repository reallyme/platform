// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature-neutral RPC identity metadata for the Hephaestus control plane.

/// Canonical protobuf service name for the Hephaestus control plane.
pub const HEPHAESTUS_RPC_SERVICE_NAME: &str = "reallyme.hephaestus.v1.HephaestusService";
/// Canonical protobuf service name for NATS orchestration planning.
pub const NATS_RPC_SERVICE_NAME: &str = "reallyme.hephaestus.v1.NatsService";
/// Canonical protobuf service name for Docker orchestration planning.
pub const DOCKER_RPC_SERVICE_NAME: &str = "reallyme.hephaestus.v1.DockerService";
/// Canonical protobuf service name for Typesense orchestration planning.
pub const TYPESENSE_RPC_SERVICE_NAME: &str = "reallyme.hephaestus.v1.TypesenseService";

/// Canonical RPC method-path metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HephaestusRpcMethodPath {
    service_name: &'static str,
    method_name: &'static str,
    path: &'static str,
}

impl HephaestusRpcMethodPath {
    /// Constructs static RPC path metadata.
    pub const fn new(
        service_name: &'static str,
        method_name: &'static str,
        path: &'static str,
    ) -> Self {
        Self {
            service_name,
            method_name,
            path,
        }
    }

    /// Returns the fully qualified protobuf service name.
    pub const fn service_name(self) -> &'static str {
        self.service_name
    }

    /// Returns the protobuf method name.
    pub const fn method_name(self) -> &'static str {
        self.method_name
    }

    /// Returns the canonical Connect/gRPC method path.
    pub const fn path(self) -> &'static str {
        self.path
    }
}

macro_rules! define_rpc {
    ($method_const:ident, $path_const:ident, $rpc_const:ident, $method_name:literal) => {
        /// Canonical protobuf method name.
        pub const $method_const: &str = $method_name;
        /// Canonical Connect/gRPC method path.
        pub const $path_const: &str =
            concat!("/reallyme.hephaestus.v1.HephaestusService/", $method_name);
        /// Canonical RPC method-path metadata.
        pub const $rpc_const: HephaestusRpcMethodPath =
            HephaestusRpcMethodPath::new(HEPHAESTUS_RPC_SERVICE_NAME, $method_const, $path_const);
    };
}

macro_rules! define_service_rpc {
    (
        $service_const:ident,
        $method_const:ident,
        $path_const:ident,
        $rpc_const:ident,
        $service_name:literal,
        $method_name:literal
    ) => {
        /// Canonical protobuf method name.
        pub const $method_const: &str = $method_name;
        /// Canonical Connect/gRPC method path.
        pub const $path_const: &str = concat!("/", $service_name, "/", $method_name);
        /// Canonical RPC method-path metadata.
        pub const $rpc_const: HephaestusRpcMethodPath =
            HephaestusRpcMethodPath::new($service_const, $method_const, $path_const);
    };
}

define_rpc!(
    HEPHAESTUS_STATUS_RPC_METHOD_NAME,
    HEPHAESTUS_STATUS_RPC_PATH,
    HEPHAESTUS_STATUS_RPC,
    "GetStatus"
);
define_rpc!(
    HEPHAESTUS_GET_DESIRED_TOPOLOGY_RPC_METHOD_NAME,
    HEPHAESTUS_GET_DESIRED_TOPOLOGY_RPC_PATH,
    HEPHAESTUS_GET_DESIRED_TOPOLOGY_RPC,
    "GetDesiredTopology"
);
define_rpc!(
    HEPHAESTUS_UPDATE_DESIRED_TOPOLOGY_RPC_METHOD_NAME,
    HEPHAESTUS_UPDATE_DESIRED_TOPOLOGY_RPC_PATH,
    HEPHAESTUS_UPDATE_DESIRED_TOPOLOGY_RPC,
    "UpdateDesiredTopology"
);
define_rpc!(
    HEPHAESTUS_PLAN_INFRA_RPC_METHOD_NAME,
    HEPHAESTUS_PLAN_INFRA_RPC_PATH,
    HEPHAESTUS_PLAN_INFRA_RPC,
    "PlanInfra"
);
define_service_rpc!(
    NATS_RPC_SERVICE_NAME,
    NATS_PLAN_NATS_DEPLOYMENT_RPC_METHOD_NAME,
    NATS_PLAN_NATS_DEPLOYMENT_RPC_PATH,
    NATS_PLAN_NATS_DEPLOYMENT_RPC,
    "reallyme.hephaestus.v1.NatsService",
    "PlanNatsDeployment"
);
define_service_rpc!(
    DOCKER_RPC_SERVICE_NAME,
    DOCKER_PLAN_DOCKER_DEPLOYMENT_RPC_METHOD_NAME,
    DOCKER_PLAN_DOCKER_DEPLOYMENT_RPC_PATH,
    DOCKER_PLAN_DOCKER_DEPLOYMENT_RPC,
    "reallyme.hephaestus.v1.DockerService",
    "PlanDockerDeployment"
);
define_service_rpc!(
    TYPESENSE_RPC_SERVICE_NAME,
    TYPESENSE_PLAN_TYPESENSE_DEPLOYMENT_RPC_METHOD_NAME,
    TYPESENSE_PLAN_TYPESENSE_DEPLOYMENT_RPC_PATH,
    TYPESENSE_PLAN_TYPESENSE_DEPLOYMENT_RPC,
    "reallyme.hephaestus.v1.TypesenseService",
    "PlanTypesenseDeployment"
);
define_rpc!(
    HEPHAESTUS_APPLY_INFRA_RPC_METHOD_NAME,
    HEPHAESTUS_APPLY_INFRA_RPC_PATH,
    HEPHAESTUS_APPLY_INFRA_RPC,
    "ApplyInfra"
);
define_rpc!(
    HEPHAESTUS_RUN_ANSIBLE_RPC_METHOD_NAME,
    HEPHAESTUS_RUN_ANSIBLE_RPC_PATH,
    HEPHAESTUS_RUN_ANSIBLE_RPC,
    "RunAnsible"
);
define_rpc!(
    HEPHAESTUS_RUN_HEALTH_CHECKS_RPC_METHOD_NAME,
    HEPHAESTUS_RUN_HEALTH_CHECKS_RPC_PATH,
    HEPHAESTUS_RUN_HEALTH_CHECKS_RPC,
    "RunHealthChecks"
);
define_rpc!(
    HEPHAESTUS_LIST_WORKFLOW_RUNS_RPC_METHOD_NAME,
    HEPHAESTUS_LIST_WORKFLOW_RUNS_RPC_PATH,
    HEPHAESTUS_LIST_WORKFLOW_RUNS_RPC,
    "ListWorkflowRuns"
);
define_rpc!(
    HEPHAESTUS_GET_WORKFLOW_RUN_RPC_METHOD_NAME,
    HEPHAESTUS_GET_WORKFLOW_RUN_RPC_PATH,
    HEPHAESTUS_GET_WORKFLOW_RUN_RPC,
    "GetWorkflowRun"
);
define_rpc!(
    HEPHAESTUS_TRIGGER_RECOVERY_RPC_METHOD_NAME,
    HEPHAESTUS_TRIGGER_RECOVERY_RPC_PATH,
    HEPHAESTUS_TRIGGER_RECOVERY_RPC,
    "TriggerRecovery"
);
define_rpc!(
    HEPHAESTUS_GET_OPERATIONAL_OVERVIEW_RPC_METHOD_NAME,
    HEPHAESTUS_GET_OPERATIONAL_OVERVIEW_RPC_PATH,
    HEPHAESTUS_GET_OPERATIONAL_OVERVIEW_RPC,
    "GetOperationalOverview"
);
define_rpc!(
    HEPHAESTUS_GET_TAILNET_OVERVIEW_RPC_METHOD_NAME,
    HEPHAESTUS_GET_TAILNET_OVERVIEW_RPC_PATH,
    HEPHAESTUS_GET_TAILNET_OVERVIEW_RPC,
    "GetTailnetOverview"
);
define_rpc!(
    HEPHAESTUS_SUBMIT_AGENT_REPORT_RPC_METHOD_NAME,
    HEPHAESTUS_SUBMIT_AGENT_REPORT_RPC_PATH,
    HEPHAESTUS_SUBMIT_AGENT_REPORT_RPC,
    "SubmitAgentReport"
);
define_rpc!(
    HEPHAESTUS_REGISTER_AGENT_RPC_METHOD_NAME,
    HEPHAESTUS_REGISTER_AGENT_RPC_PATH,
    HEPHAESTUS_REGISTER_AGENT_RPC,
    "RegisterAgent"
);
define_rpc!(
    HEPHAESTUS_QUEUE_AGENT_ACTION_RPC_METHOD_NAME,
    HEPHAESTUS_QUEUE_AGENT_ACTION_RPC_PATH,
    HEPHAESTUS_QUEUE_AGENT_ACTION_RPC,
    "QueueAgentAction"
);
define_rpc!(
    HEPHAESTUS_POLL_AGENT_ACTIONS_RPC_METHOD_NAME,
    HEPHAESTUS_POLL_AGENT_ACTIONS_RPC_PATH,
    HEPHAESTUS_POLL_AGENT_ACTIONS_RPC,
    "PollAgentActions"
);
define_rpc!(
    HEPHAESTUS_LIST_AGENT_ACTION_RECORDS_RPC_METHOD_NAME,
    HEPHAESTUS_LIST_AGENT_ACTION_RECORDS_RPC_PATH,
    HEPHAESTUS_LIST_AGENT_ACTION_RECORDS_RPC,
    "ListAgentActionRecords"
);
define_rpc!(
    HEPHAESTUS_COMPLETE_AGENT_ACTION_RPC_METHOD_NAME,
    HEPHAESTUS_COMPLETE_AGENT_ACTION_RPC_PATH,
    HEPHAESTUS_COMPLETE_AGENT_ACTION_RPC,
    "CompleteAgentAction"
);
define_rpc!(
    HEPHAESTUS_RESIZE_NODE_RPC_METHOD_NAME,
    HEPHAESTUS_RESIZE_NODE_RPC_PATH,
    HEPHAESTUS_RESIZE_NODE_RPC,
    "ResizeNode"
);
define_rpc!(
    HEPHAESTUS_CREATE_IPV4_EGRESS_GATEWAY_RPC_METHOD_NAME,
    HEPHAESTUS_CREATE_IPV4_EGRESS_GATEWAY_RPC_PATH,
    HEPHAESTUS_CREATE_IPV4_EGRESS_GATEWAY_RPC,
    "CreateIpv4EgressGateway"
);
define_rpc!(
    HEPHAESTUS_RESOLVE_AGENT_SECRET_RPC_METHOD_NAME,
    HEPHAESTUS_RESOLVE_AGENT_SECRET_RPC_PATH,
    HEPHAESTUS_RESOLVE_AGENT_SECRET_RPC,
    "ResolveAgentSecret"
);
define_rpc!(
    HEPHAESTUS_SUBMIT_AGENT_BOOT_REPORT_RPC_METHOD_NAME,
    HEPHAESTUS_SUBMIT_AGENT_BOOT_REPORT_RPC_PATH,
    HEPHAESTUS_SUBMIT_AGENT_BOOT_REPORT_RPC,
    "SubmitAgentBootReport"
);
define_rpc!(
    HEPHAESTUS_LIST_AGENT_REPORTS_RPC_METHOD_NAME,
    HEPHAESTUS_LIST_AGENT_REPORTS_RPC_PATH,
    HEPHAESTUS_LIST_AGENT_REPORTS_RPC,
    "ListAgentReports"
);
define_rpc!(
    HEPHAESTUS_GET_AGENT_REPORT_BY_SERVER_ID_RPC_METHOD_NAME,
    HEPHAESTUS_GET_AGENT_REPORT_BY_SERVER_ID_RPC_PATH,
    HEPHAESTUS_GET_AGENT_REPORT_BY_SERVER_ID_RPC,
    "GetAgentReportByServerId"
);
define_rpc!(
    HEPHAESTUS_LIST_NODE_ACTUAL_STATES_RPC_METHOD_NAME,
    HEPHAESTUS_LIST_NODE_ACTUAL_STATES_RPC_PATH,
    HEPHAESTUS_LIST_NODE_ACTUAL_STATES_RPC,
    "ListNodeActualStates"
);
define_rpc!(
    HEPHAESTUS_GET_NODE_ACTUAL_STATE_BY_SERVER_ID_RPC_METHOD_NAME,
    HEPHAESTUS_GET_NODE_ACTUAL_STATE_BY_SERVER_ID_RPC_PATH,
    HEPHAESTUS_GET_NODE_ACTUAL_STATE_BY_SERVER_ID_RPC,
    "GetNodeActualStateByServerId"
);
define_rpc!(
    HEPHAESTUS_LIST_SERVER_INVENTORY_RPC_METHOD_NAME,
    HEPHAESTUS_LIST_SERVER_INVENTORY_RPC_PATH,
    HEPHAESTUS_LIST_SERVER_INVENTORY_RPC,
    "ListServerInventory"
);
define_rpc!(
    HEPHAESTUS_GET_NODE_LIVE_REPORT_BY_SERVER_ID_RPC_METHOD_NAME,
    HEPHAESTUS_GET_NODE_LIVE_REPORT_BY_SERVER_ID_RPC_PATH,
    HEPHAESTUS_GET_NODE_LIVE_REPORT_BY_SERVER_ID_RPC,
    "GetNodeLiveReportByServerId"
);
