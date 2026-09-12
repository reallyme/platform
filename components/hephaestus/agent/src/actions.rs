// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed node-local actions the agent may execute.
//!
//! The agent is intentionally not a remote shell. It accepts only generated
//! protobuf action messages from Hephaestus and maps them to fixed local
//! commands with validated arguments. This keeps the emergency control plane
//! useful without turning every production host into an arbitrary command
//! runner.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1 as pb;
use reallyme_hephaestus_domain::HephaestusAgentReport;
use tokio::process::Command;
use tokio::time::timeout;

use crate::audit::{AgentActionAuditLog, AgentAuditActionMetadata};
use crate::config::HephaestusAgentConfig;
use crate::docker_engine;
use crate::docker_service::{
    DockerServiceSecretResolver, DockerServiceSpec, configure_docker_service,
};
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};
use crate::foundationdb_service::{
    FoundationDbServiceError, FoundationDbServiceSpec, configure_foundationdb_service,
};
use crate::tailscale;

const SYSTEMCTL_BINARY: &str = "/bin/systemctl";
const REBOOT_REQUIRED_PATH: &str = "/var/run/reboot-required";
const ACTION_EXECUTION_TIMEOUT_MULTIPLIER: u32 = 4;

/// Minimal control-plane surface needed while applying a polled action batch.
///
/// Keeping this boundary small lets tests exercise partial reporting failures
/// without substituting the full generated Connect client.
pub trait AgentActionControlPlane: DockerServiceSecretResolver + Send + Sync {
    /// Resolves narrowly scoped Docker host authority immediately before use.
    ///
    /// The empty default preserves rolling upgrades with controllers that do
    /// not yet expose the authority service and grants no additional device or
    /// Linux-capability access.
    fn resolve_docker_runtime_authority<'a>(
        &'a self,
        _receipt: &'a AgentActionReceipt,
        _service_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = AgentResult<Option<pb::DockerRuntimeAuthority>>> + Send + 'a>>
    {
        Box::pin(async { Ok(None) })
    }

    /// Sends an immediate host report requested by an action.
    fn submit_agent_report<'a>(
        &'a self,
        report: &'a HephaestusAgentReport,
    ) -> Pin<Box<dyn Future<Output = AgentResult<()>> + Send + 'a>>;

    /// Marks a locally processed action complete in central Hephaestus.
    fn complete_agent_action<'a>(
        &'a self,
        receipt: &'a AgentActionReceipt,
        outcome: AgentActionOutcome,
    ) -> Pin<Box<dyn Future<Output = AgentResult<()>> + Send + 'a>>;
}

impl AgentActionControlPlane for crate::control_plane::HephaestusControlPlaneClient {
    fn resolve_docker_runtime_authority<'a>(
        &'a self,
        receipt: &'a AgentActionReceipt,
        service_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = AgentResult<Option<pb::DockerRuntimeAuthority>>> + Send + 'a>>
    {
        Box::pin(
            crate::control_plane::HephaestusControlPlaneClient::resolve_docker_runtime_authority(
                self, receipt, service_id,
            ),
        )
    }

    fn submit_agent_report<'a>(
        &'a self,
        report: &'a HephaestusAgentReport,
    ) -> Pin<Box<dyn Future<Output = AgentResult<()>> + Send + 'a>> {
        Box::pin(
            crate::control_plane::HephaestusControlPlaneClient::submit_agent_report(self, report),
        )
    }

    fn complete_agent_action<'a>(
        &'a self,
        receipt: &'a AgentActionReceipt,
        outcome: AgentActionOutcome,
    ) -> Pin<Box<dyn Future<Output = AgentResult<()>> + Send + 'a>> {
        Box::pin(
            crate::control_plane::HephaestusControlPlaneClient::complete_agent_action(
                self, receipt, outcome,
            ),
        )
    }
}

/// Result of a local action attempt, mapped back to protobuf completion status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentActionOutcome {
    status: pb::AgentActionStatus,
    reason: pb::AgentActionResultReason,
    report_now: bool,
}

impl AgentActionOutcome {
    /// Successful action completion.
    pub const fn succeeded(report_now: bool) -> Self {
        Self {
            status: pb::AgentActionStatus::AGENT_ACTION_STATUS_SUCCEEDED,
            reason: pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_OK,
            report_now,
        }
    }

    /// Rejected action completion.
    pub const fn rejected(reason: pb::AgentActionResultReason) -> Self {
        Self {
            status: pb::AgentActionStatus::AGENT_ACTION_STATUS_REJECTED,
            reason,
            report_now: false,
        }
    }

    /// Failed action completion.
    pub const fn failed(reason: pb::AgentActionResultReason) -> Self {
        Self {
            status: pb::AgentActionStatus::AGENT_ACTION_STATUS_FAILED,
            reason,
            report_now: false,
        }
    }

    /// Protobuf status.
    pub const fn status(self) -> pb::AgentActionStatus {
        self.status
    }

    /// Protobuf reason.
    pub const fn reason(self) -> pb::AgentActionResultReason {
        self.reason
    }

    /// Whether the runtime should emit a fresh agent report immediately.
    pub const fn should_report_now(self) -> bool {
        self.report_now
    }
}

/// Validated action metadata used when reporting completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentActionReceipt {
    action_id: String,
    node_id: String,
    idempotency_key: String,
    desired_generation: u64,
}

impl AgentActionReceipt {
    /// Returns the action id.
    pub fn action_id(&self) -> &str {
        self.action_id.as_str()
    }

    /// Returns the target node id.
    pub fn node_id(&self) -> &str {
        self.node_id.as_str()
    }

    /// Returns the idempotency key.
    pub fn idempotency_key(&self) -> &str {
        self.idempotency_key.as_str()
    }

    /// Returns the desired state generation attached to the action.
    pub const fn desired_generation(&self) -> u64 {
        self.desired_generation
    }
}

/// Fully validated local action.
#[derive(Debug, Clone)]
pub struct ExecutableAgentAction {
    receipt: AgentActionReceipt,
    kind: ExecutableAgentActionKind,
}

impl ExecutableAgentAction {
    /// Validates a protobuf action for this node and converts it to a local plan.
    pub fn from_proto(
        mut action: pb::AgentAction,
        expected_node_id: &str,
        now_unix_secs: u64,
    ) -> Result<Self, AgentActionOutcome> {
        if !is_safe_token(action.action_id.as_str())
            || !is_safe_token(action.node_id.as_str())
            || !is_safe_token(action.idempotency_key.as_str())
            || action.desired_generation == 0
            || action.deadline_unix_secs == 0
        {
            return Err(AgentActionOutcome::rejected(
                pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
            ));
        }
        if action.node_id != expected_node_id {
            return Err(AgentActionOutcome::rejected(
                pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_WRONG_NODE,
            ));
        }
        if action.deadline_unix_secs < now_unix_secs {
            return Err(AgentActionOutcome::rejected(
                pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_EXPIRED,
            ));
        }

        let Some(kind) = action.kind.as_known() else {
            return Err(AgentActionOutcome::rejected(
                pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
            ));
        };
        let executable_kind = match kind {
            pb::AgentActionKind::AGENT_ACTION_KIND_UNSPECIFIED => {
                return Err(AgentActionOutcome::rejected(
                    pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
                ));
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_REPORT_NOW => {
                ExecutableAgentActionKind::ReportNow
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_PULL_IMAGE => {
                let docker = take_required_message(&mut action.docker)?;
                if !is_safe_image_ref(docker.image_ref.as_str()) {
                    return Err(AgentActionOutcome::rejected(
                        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
                    ));
                }
                ExecutableAgentActionKind::PullImage {
                    image_ref: docker.image_ref,
                }
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_START_SERVICE => {
                let systemd = take_required_message(&mut action.systemd)?;
                let unit_name = validate_unit_name(systemd.unit_name)?;
                ExecutableAgentActionKind::StartService { unit_name }
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_STOP_SERVICE => {
                let systemd = take_required_message(&mut action.systemd)?;
                let unit_name = validate_unit_name(systemd.unit_name)?;
                ExecutableAgentActionKind::StopService { unit_name }
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_RESTART_SERVICE => {
                let systemd = take_required_message(&mut action.systemd)?;
                let unit_name = validate_unit_name(systemd.unit_name)?;
                ExecutableAgentActionKind::RestartService { unit_name }
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_DRAIN_TAILSCALE_SERVICE => {
                let tailscale_service = take_required_message(&mut action.tailscale_service)?;
                let service_name = validate_service_name(tailscale_service.service_name)?;
                ExecutableAgentActionKind::DrainTailscaleService { service_name }
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_ADVERTISE_TAILSCALE_SERVICE => {
                let tailscale_service = take_required_message(&mut action.tailscale_service)?;
                let service_name = validate_service_name(tailscale_service.service_name)?;
                ExecutableAgentActionKind::AdvertiseTailscaleService { service_name }
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_REBOOT_HOST => {
                let reboot = take_required_message(&mut action.reboot)?;
                ExecutableAgentActionKind::RebootHost {
                    require_reboot_required_file: reboot.require_reboot_required_file,
                }
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_RESTART_CONTAINER => {
                let docker = take_required_message(&mut action.docker)?;
                let container_name = validate_container_name(docker.container_name)?;
                ExecutableAgentActionKind::RestartContainer { container_name }
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_STOP_CONTAINER => {
                let docker = take_required_message(&mut action.docker)?;
                let container_name = validate_container_name(docker.container_name)?;
                ExecutableAgentActionKind::StopContainer { container_name }
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_CONFIGURE_DOCKER_SERVICE => {
                let docker_service = take_required_message(&mut action.docker_service)?;
                let service = DockerServiceSpec::from_proto(docker_service).map_err(|_error| {
                    AgentActionOutcome::rejected(
                        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
                    )
                })?;
                ExecutableAgentActionKind::ConfigureDockerService {
                    service: Box::new(service),
                }
            }
            pb::AgentActionKind::AGENT_ACTION_KIND_CONFIGURE_FOUNDATIONDB_SERVICE => {
                let foundationdb_service = take_required_message(&mut action.foundationdb_service)?;
                let service = FoundationDbServiceSpec::from_proto(foundationdb_service).map_err(
                    |_error| {
                        AgentActionOutcome::rejected(
                            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
                        )
                    },
                )?;
                ExecutableAgentActionKind::ConfigureFoundationDbService {
                    service: Box::new(service),
                }
            }
        };

        Ok(Self {
            receipt: AgentActionReceipt {
                action_id: action.action_id,
                node_id: expected_node_id.to_owned(),
                idempotency_key: action.idempotency_key,
                desired_generation: action.desired_generation,
            },
            kind: executable_kind,
        })
    }

    /// Returns validated receipt fields used for completion.
    pub const fn receipt(&self) -> &AgentActionReceipt {
        &self.receipt
    }

    /// Returns the redacted action kind label used for local audit records.
    pub const fn audit_kind(&self) -> &'static str {
        self.kind.audit_label()
    }

    async fn apply_docker_runtime_authority(
        &mut self,
        control_plane: &impl AgentActionControlPlane,
    ) -> Result<(), AgentActionOutcome> {
        let ExecutableAgentActionKind::ConfigureDockerService { service } = &mut self.kind else {
            return Ok(());
        };
        let authority = control_plane
            .resolve_docker_runtime_authority(&self.receipt, service.service_id())
            .await
            .map_err(|_error| {
                AgentActionOutcome::failed(
                    pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED,
                )
            })?;
        service
            .apply_runtime_authority(authority)
            .map_err(|_error| {
                AgentActionOutcome::rejected(
                    pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
                )
            })
    }

    /// Executes the validated action locally.
    pub async fn execute(
        &self,
        config: &HephaestusAgentConfig,
        secret_resolver: &impl DockerServiceSecretResolver,
    ) -> AgentActionOutcome {
        match &self.kind {
            ExecutableAgentActionKind::ReportNow => AgentActionOutcome::succeeded(true),
            ExecutableAgentActionKind::PullImage { image_ref } => {
                match docker_engine::pull_image(image_ref.as_str(), config.command_timeout()).await
                {
                    Ok(()) => AgentActionOutcome::succeeded(false),
                    Err(_error) => AgentActionOutcome::failed(
                        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED,
                    ),
                }
            }
            ExecutableAgentActionKind::RestartContainer { container_name } => {
                match docker_engine::restart_container(
                    container_name.as_str(),
                    config.command_timeout(),
                )
                .await
                {
                    Ok(()) => AgentActionOutcome::succeeded(false),
                    Err(_error) => AgentActionOutcome::failed(
                        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED,
                    ),
                }
            }
            ExecutableAgentActionKind::StopContainer { container_name } => {
                match docker_engine::stop_container(
                    container_name.as_str(),
                    config.command_timeout(),
                )
                .await
                {
                    Ok(()) => AgentActionOutcome::succeeded(false),
                    Err(_error) => AgentActionOutcome::failed(
                        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED,
                    ),
                }
            }
            ExecutableAgentActionKind::StartService { unit_name } => {
                systemctl_action("start", unit_name.as_str(), config.command_timeout()).await
            }
            ExecutableAgentActionKind::StopService { unit_name } => {
                systemctl_action("stop", unit_name.as_str(), config.command_timeout()).await
            }
            ExecutableAgentActionKind::RestartService { unit_name } => {
                systemctl_action("restart", unit_name.as_str(), config.command_timeout()).await
            }
            ExecutableAgentActionKind::DrainTailscaleService { service_name } => {
                match tailscale::drain_service(service_name.as_str(), config.command_timeout()).await
                {
                    Ok(()) => AgentActionOutcome::succeeded(false),
                    Err(_error) => AgentActionOutcome::failed(
                        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED,
                    ),
                }
            }
            ExecutableAgentActionKind::AdvertiseTailscaleService { service_name } => {
                match tailscale::advertise_service(service_name.as_str(), config.command_timeout())
                    .await
                {
                    Ok(()) => AgentActionOutcome::succeeded(false),
                    Err(_error) => AgentActionOutcome::failed(
                        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED,
                    ),
                }
            }
            ExecutableAgentActionKind::RebootHost {
                require_reboot_required_file,
            } => {
                if *require_reboot_required_file && !Path::new(REBOOT_REQUIRED_PATH).exists() {
                    return AgentActionOutcome::rejected(
                        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_VALIDATION_FAILED,
                    );
                }
                run_fixed_command(SYSTEMCTL_BINARY, ["reboot"], config.command_timeout()).await
            }
            ExecutableAgentActionKind::ConfigureDockerService { service } => {
                match configure_docker_service(
                    service,
                    self.receipt(),
                    config.command_timeout(),
                    secret_resolver,
                )
                .await
                {
                    Ok(()) => match tailscale::apply_service_configs(
                        service.tailscale_services(),
                        config.command_timeout(),
                    )
                    .await
                    {
                        Ok(()) => AgentActionOutcome::succeeded(true),
                        Err(_error) => AgentActionOutcome::failed(
                            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED,
                        ),
                    },
                    Err(_error) => AgentActionOutcome::failed(
                        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED,
                    ),
                }
            }
            ExecutableAgentActionKind::ConfigureFoundationDbService { service } => {
                match configure_foundationdb_service(service, config.command_timeout()).await {
                    Ok(()) => AgentActionOutcome::succeeded(true),
                    Err(error) => {
                        tracing::warn!(
                            reason = ?error,
                            "failed to configure FoundationDB service"
                        );
                        AgentActionOutcome::failed(foundationdb_action_result_reason(error))
                    }
                }
            }
        }
    }
}

fn foundationdb_action_result_reason(
    error: FoundationDbServiceError,
) -> pb::AgentActionResultReason {
    match error {
        FoundationDbServiceError::InvalidAddress
        | FoundationDbServiceError::InvalidTopology
        | FoundationDbServiceError::InvalidIdentifier
        | FoundationDbServiceError::InvalidProcessClass
        | FoundationDbServiceError::InvalidRedundancyMode
        | FoundationDbServiceError::InvalidStorageEngine
        | FoundationDbServiceError::InvalidClusterOperation => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_VALIDATION_FAILED
        }
        FoundationDbServiceError::Filesystem | FoundationDbServiceError::LocalCommand => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED
        }
        FoundationDbServiceError::ServiceStartFailed => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_SERVICE_START_FAILED
        }
        FoundationDbServiceError::ConfigureNewFailed => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_CONFIGURE_NEW_FAILED
        }
        FoundationDbServiceError::StatusUnavailable => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_STATUS_UNAVAILABLE
        }
        FoundationDbServiceError::CoordinatorsUnavailable => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_COORDINATORS_UNAVAILABLE
        }
        FoundationDbServiceError::DatabaseNotConfigured => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_DATABASE_NOT_CONFIGURED
        }
        FoundationDbServiceError::DatabaseUnavailable => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_DATABASE_UNAVAILABLE
        }
        FoundationDbServiceError::ConfigurationInvalid => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_CONFIGURATION_INVALID
        }
        FoundationDbServiceError::StaleClusterFile => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_STALE_CLUSTER_FILE
        }
        FoundationDbServiceError::RecruitmentPending => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_RECRUITMENT_PENDING
        }
        FoundationDbServiceError::RecoveryInProgress => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_RECOVERY_IN_PROGRESS
        }
        FoundationDbServiceError::DataMovementInProgress => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_DATA_MOVEMENT_IN_PROGRESS
        }
        FoundationDbServiceError::CoordinatorQuorumLost => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_COORDINATOR_QUORUM_LOST
        }
        FoundationDbServiceError::InsufficientStorageForRedundancy => {
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_INSUFFICIENT_STORAGE_FOR_REDUNDANCY
        }
    }
}

#[derive(Debug, Clone)]
enum ExecutableAgentActionKind {
    ReportNow,
    PullImage {
        image_ref: String,
    },
    RestartContainer {
        container_name: String,
    },
    StopContainer {
        container_name: String,
    },
    StartService {
        unit_name: String,
    },
    StopService {
        unit_name: String,
    },
    RestartService {
        unit_name: String,
    },
    DrainTailscaleService {
        service_name: String,
    },
    AdvertiseTailscaleService {
        service_name: String,
    },
    RebootHost {
        require_reboot_required_file: bool,
    },
    ConfigureDockerService {
        service: Box<DockerServiceSpec>,
    },
    ConfigureFoundationDbService {
        service: Box<FoundationDbServiceSpec>,
    },
}

impl ExecutableAgentActionKind {
    const fn audit_label(&self) -> &'static str {
        match self {
            Self::ReportNow => "report_now",
            Self::PullImage { .. } => "pull_image",
            Self::RestartContainer { .. } => "restart_container",
            Self::StopContainer { .. } => "stop_container",
            Self::StartService { .. } => "start_service",
            Self::StopService { .. } => "stop_service",
            Self::RestartService { .. } => "restart_service",
            Self::DrainTailscaleService { .. } => "drain_tailscale_service",
            Self::AdvertiseTailscaleService { .. } => "advertise_tailscale_service",
            Self::RebootHost { .. } => "reboot_host",
            Self::ConfigureDockerService { .. } => "configure_docker_service",
            Self::ConfigureFoundationDbService { .. } => "configure_foundationdb_service",
        }
    }
}

/// Executes all polled actions and reports completions.
pub async fn execute_polled_actions(
    config: &HephaestusAgentConfig,
    client: &impl AgentActionControlPlane,
    actions: Vec<pb::AgentAction>,
) -> AgentResult<u64> {
    let now = current_unix_seconds()?;
    let audit_log = AgentActionAuditLog::new(config.audit_log_path())?;
    let mut observed_generation = 0u64;
    for action in actions {
        let replay_guard_generation = replay_guard_generation_from_action(&action);
        let audit_metadata = AgentAuditActionMetadata::from_action(&action, config.server_id());
        audit_log.record_received(&audit_metadata).await?;
        let rejected_receipt = rejected_receipt_from_proto(&action, config.server_id());
        match ExecutableAgentAction::from_proto(action, config.server_id(), now) {
            Ok(mut executable) => {
                let audit_metadata = AgentAuditActionMetadata::from_receipt(
                    executable.receipt(),
                    executable.audit_kind(),
                );
                audit_log.record_accepted(&audit_metadata).await?;
                audit_log.record_started(&audit_metadata).await?;
                let execution = async {
                    match executable.apply_docker_runtime_authority(client).await {
                        Ok(()) => executable.execute(config, client).await,
                        Err(outcome) => outcome,
                    }
                };
                let outcome = match timeout(
                    agent_action_execution_timeout(config.command_timeout()),
                    execution,
                )
                .await
                {
                    Ok(outcome) => outcome,
                    Err(_elapsed) => AgentActionOutcome::failed(
                        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED,
                    ),
                };
                audit_log.record_completed(&audit_metadata, outcome).await?;
                let completion_result = client
                    .complete_agent_action(executable.receipt(), outcome)
                    .await;
                match completion_result {
                    Ok(()) => {
                        audit_log
                            .record_completion_reported(&audit_metadata, outcome)
                            .await?;
                    }
                    Err(error) => {
                        audit_log
                            .record_completion_report_failed(&audit_metadata, outcome)
                            .await?;
                        // Local actions are intentionally idempotent. If the
                        // control plane is temporarily unavailable, keep the
                        // agent alive and let central reconciliation decide
                        // whether to redeliver the action generation.
                        tracing::warn!(
                            reason = ?error.reason(),
                            action_kind = audit_metadata.action_kind(),
                            "failed to report agent action completion"
                        );
                    }
                }
                observed_generation =
                    observed_generation.max(executable.receipt().desired_generation());
            }
            Err(outcome) => {
                audit_log.record_rejected(&audit_metadata, outcome).await?;
                if let Some(receipt) = rejected_receipt {
                    let completion_result = client.complete_agent_action(&receipt, outcome).await;
                    match completion_result {
                        Ok(()) => {
                            audit_log
                                .record_completion_reported(&audit_metadata, outcome)
                                .await?;
                        }
                        Err(error) => {
                            audit_log
                                .record_completion_report_failed(&audit_metadata, outcome)
                                .await?;
                            tracing::warn!(
                                reason = ?error.reason(),
                                action_kind = audit_metadata.action_kind(),
                                "failed to report rejected agent action"
                            );
                        }
                    }
                    observed_generation = observed_generation.max(receipt.desired_generation());
                } else {
                    observed_generation = observed_generation.max(replay_guard_generation);
                }
            }
        }
    }
    Ok(observed_generation)
}

fn replay_guard_generation_from_action(action: &pb::AgentAction) -> u64 {
    action.desired_generation
}

fn rejected_receipt_from_proto(
    action: &pb::AgentAction,
    expected_node_id: &str,
) -> Option<AgentActionReceipt> {
    if !is_safe_token(action.action_id.as_str())
        || !is_safe_token(action.idempotency_key.as_str())
        || action.desired_generation == 0
    {
        return None;
    }
    Some(AgentActionReceipt {
        action_id: action.action_id.clone(),
        node_id: expected_node_id.to_owned(),
        idempotency_key: action.idempotency_key.clone(),
        desired_generation: action.desired_generation,
    })
}

fn agent_action_execution_timeout(command_timeout: Duration) -> Duration {
    match command_timeout.checked_mul(ACTION_EXECUTION_TIMEOUT_MULTIPLIER) {
        Some(timeout) => timeout,
        None => Duration::MAX,
    }
}

fn take_required_message<T: Default, P: buffa::ProtoBox<T>>(
    field: &mut buffa::MessageField<T, P>,
) -> Result<T, AgentActionOutcome> {
    field.take().ok_or_else(|| {
        AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
        )
    })
}

async fn systemctl_action(
    verb: &'static str,
    unit_name: &str,
    command_timeout: Duration,
) -> AgentActionOutcome {
    run_fixed_command(SYSTEMCTL_BINARY, [verb, "--", unit_name], command_timeout).await
}

async fn run_fixed_command<const N: usize>(
    binary: &str,
    args: [&str; N],
    command_timeout: Duration,
) -> AgentActionOutcome {
    let mut command = Command::new(binary);
    command.args(args);
    let result = timeout(command_timeout, crate::command::output(&mut command)).await;
    match result {
        Ok(Ok(output)) if output.status.success() => AgentActionOutcome::succeeded(false),
        Ok(Ok(_)) | Ok(Err(_)) | Err(_) => AgentActionOutcome::failed(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED,
        ),
    }
}

fn current_unix_seconds() -> AgentResult<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))
}

fn validate_unit_name(value: String) -> Result<String, AgentActionOutcome> {
    if value.is_empty()
        || value.starts_with('-')
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'@'))
    {
        return Err(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
        ));
    }
    Ok(value)
}

fn validate_service_name(value: String) -> Result<String, AgentActionOutcome> {
    if value.is_empty()
        || value.starts_with('-')
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':'))
    {
        return Err(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
        ));
    }
    Ok(value)
}

fn validate_container_name(value: String) -> Result<String, AgentActionOutcome> {
    if value.is_empty()
        || value.starts_with('-')
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION,
        ));
    }
    Ok(value)
}

fn is_safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn is_safe_image_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/' | b'@' | b'+')
        })
}

#[cfg(test)]
mod tests;
