// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Durable local audit records for node-local agent actions.
//!
//! The control plane remains the source of truth, but the node must retain a
//! local, append-only record of every action it accepted, rejected, attempted,
//! and reported. These records intentionally contain identifiers and enum-like
//! state only; action payloads, rendered secrets, and environment values are
//! never serialized to the audit file.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1 as pb;
use serde::Serialize;
use tokio::io::AsyncWriteExt as _;

use crate::actions::{AgentActionOutcome, AgentActionReceipt};
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

const AUDIT_SCHEMA_VERSION: u32 = 1;

/// Append-only local audit log for agent action lifecycle events.
#[derive(Debug, Clone)]
pub struct AgentActionAuditLog {
    path: PathBuf,
}

impl AgentActionAuditLog {
    /// Constructs an audit writer for an absolute path.
    pub fn new(path: &Path) -> AgentResult<Self> {
        validate_audit_path(path)?;
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Records that an action was observed in a poll response.
    pub async fn record_received(&self, metadata: &AgentAuditActionMetadata) -> AgentResult<()> {
        self.append(AuditEvent::Received, metadata, None).await
    }

    /// Records that an action passed local validation and is eligible to run.
    pub async fn record_accepted(&self, metadata: &AgentAuditActionMetadata) -> AgentResult<()> {
        self.append(AuditEvent::Accepted, metadata, None).await
    }

    /// Records that local execution is about to begin.
    pub async fn record_started(&self, metadata: &AgentAuditActionMetadata) -> AgentResult<()> {
        self.append(AuditEvent::Started, metadata, None).await
    }

    /// Records the local action result before remote completion reporting.
    pub async fn record_completed(
        &self,
        metadata: &AgentAuditActionMetadata,
        outcome: AgentActionOutcome,
    ) -> AgentResult<()> {
        self.append(AuditEvent::Completed, metadata, Some(outcome))
            .await
    }

    /// Records that the central control plane accepted the completion report.
    pub async fn record_completion_reported(
        &self,
        metadata: &AgentAuditActionMetadata,
        outcome: AgentActionOutcome,
    ) -> AgentResult<()> {
        self.append(AuditEvent::CompletionReported, metadata, Some(outcome))
            .await
    }

    /// Records that remote completion reporting failed.
    pub async fn record_completion_report_failed(
        &self,
        metadata: &AgentAuditActionMetadata,
        outcome: AgentActionOutcome,
    ) -> AgentResult<()> {
        self.append(AuditEvent::CompletionReportFailed, metadata, Some(outcome))
            .await
    }

    /// Records that a malformed, wrong-node, or expired action was rejected.
    pub async fn record_rejected(
        &self,
        metadata: &AgentAuditActionMetadata,
        outcome: AgentActionOutcome,
    ) -> AgentResult<()> {
        self.append(AuditEvent::Rejected, metadata, Some(outcome))
            .await
    }

    async fn append(
        &self,
        event: AuditEvent,
        metadata: &AgentAuditActionMetadata,
        outcome: Option<AgentActionOutcome>,
    ) -> AgentResult<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
        tokio::fs::create_dir_all(parent).await.map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
        })?;

        let record = AgentAuditRecord {
            schema_version: AUDIT_SCHEMA_VERSION,
            event_unix_secs: current_unix_seconds()?,
            event,
            node_id: metadata.node_id.as_str(),
            action_id: metadata.action_id.as_deref(),
            idempotency_key: metadata.idempotency_key.as_deref(),
            desired_generation: metadata.desired_generation,
            action_kind: metadata.action_kind,
            target_node_matches: metadata.target_node_matches,
            status: outcome.map(action_status_label),
            reason: outcome.map(action_reason_label),
        };
        let mut bytes = serde_json::to_vec(&record).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
        })?;
        bytes.push(b'\n');

        let mut options = tokio::fs::OpenOptions::new();
        options.create(true).append(true);
        set_audit_creation_mode(&mut options);
        // Reassert 0o600 on freshly created files as a defensive measure.
        // OpenOptions::mode(0o600) already requests this for creation; pre-existing
        // files are trusted in our root-owned state directory.
        let file_pre_exists = tokio::fs::metadata(self.path.as_path()).await.is_ok();
        let mut file = options.open(self.path.as_path()).await.map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
        })?;
        if !file_pre_exists {
            set_audit_permissions(self.path.as_path()).await?;
        }
        file.write_all(bytes.as_slice()).await.map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
        })?;
        file.sync_data().await.map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
        })
    }
}

#[cfg(unix)]
fn set_audit_creation_mode(options: &mut tokio::fs::OpenOptions) {
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_audit_creation_mode(_options: &mut tokio::fs::OpenOptions) {}

/// Redacted metadata safe to persist in the local audit log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentAuditActionMetadata {
    node_id: String,
    action_id: Option<String>,
    idempotency_key: Option<String>,
    desired_generation: u64,
    action_kind: &'static str,
    target_node_matches: bool,
}

impl AgentAuditActionMetadata {
    /// Creates redacted metadata from a protobuf action before validation.
    pub fn from_action(action: &pb::AgentAction, expected_node_id: &str) -> Self {
        Self {
            node_id: expected_node_id.to_owned(),
            action_id: safe_token(action.action_id.as_str()),
            idempotency_key: safe_token(action.idempotency_key.as_str()),
            desired_generation: action.desired_generation,
            action_kind: action_kind_label(action.kind.as_known()),
            target_node_matches: action.node_id == expected_node_id,
        }
    }

    /// Creates metadata from a validated receipt.
    pub fn from_receipt(receipt: &AgentActionReceipt, action_kind: &'static str) -> Self {
        Self {
            node_id: receipt.node_id().to_owned(),
            action_id: Some(receipt.action_id().to_owned()),
            idempotency_key: Some(receipt.idempotency_key().to_owned()),
            desired_generation: receipt.desired_generation(),
            action_kind,
            target_node_matches: true,
        }
    }

    /// Returns the redacted action kind label.
    pub const fn action_kind(&self) -> &'static str {
        self.action_kind
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum AuditEvent {
    Received,
    Accepted,
    Started,
    Completed,
    Rejected,
    CompletionReported,
    CompletionReportFailed,
}

#[derive(Serialize)]
struct AgentAuditRecord<'a> {
    schema_version: u32,
    event_unix_secs: u64,
    event: AuditEvent,
    node_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    action_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idempotency_key: Option<&'a str>,
    desired_generation: u64,
    action_kind: &'static str,
    target_node_matches: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
}

fn current_unix_seconds() -> AgentResult<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))
}

fn validate_audit_path(path: &Path) -> AgentResult<()> {
    if !path.is_absolute() {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ));
    }
    Ok(())
}

fn safe_token(value: &str) -> Option<String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        None
    } else {
        Some(value.to_owned())
    }
}

fn action_kind_label(value: Option<pb::AgentActionKind>) -> &'static str {
    match value {
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_REPORT_NOW) => "report_now",
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_PULL_IMAGE) => "pull_image",
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_START_SERVICE) => "start_service",
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_STOP_SERVICE) => "stop_service",
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_RESTART_SERVICE) => "restart_service",
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_DRAIN_TAILSCALE_SERVICE) => {
            "drain_tailscale_service"
        }
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_ADVERTISE_TAILSCALE_SERVICE) => {
            "advertise_tailscale_service"
        }
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_REBOOT_HOST) => "reboot_host",
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_RESTART_CONTAINER) => "restart_container",
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_STOP_CONTAINER) => "stop_container",
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_CONFIGURE_DOCKER_SERVICE) => {
            "configure_docker_service"
        }
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_CONFIGURE_FOUNDATIONDB_SERVICE) => {
            "configure_foundationdb_service"
        }
        Some(pb::AgentActionKind::AGENT_ACTION_KIND_UNSPECIFIED) => "unspecified",
        None => "unknown",
    }
}

fn action_status_label(outcome: AgentActionOutcome) -> &'static str {
    match outcome.status() {
        pb::AgentActionStatus::AGENT_ACTION_STATUS_SUCCEEDED => "succeeded",
        pb::AgentActionStatus::AGENT_ACTION_STATUS_FAILED => "failed",
        pb::AgentActionStatus::AGENT_ACTION_STATUS_REJECTED => "rejected",
        pb::AgentActionStatus::AGENT_ACTION_STATUS_UNSPECIFIED => "unspecified",
    }
}

fn action_reason_label(outcome: AgentActionOutcome) -> &'static str {
    match outcome.reason() {
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_OK => "ok",
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION => "invalid_action",
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_EXPIRED => "expired",
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_WRONG_NODE => "wrong_node",
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_COMMAND_FAILED => {
            "local_command_failed"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_LOCAL_VALIDATION_FAILED => {
            "local_validation_failed"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_SERVICE_START_FAILED => {
            "foundationdb_service_start_failed"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_CONFIGURE_NEW_FAILED => {
            "foundationdb_configure_new_failed"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_STATUS_UNAVAILABLE => {
            "foundationdb_status_unavailable"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_COORDINATORS_UNAVAILABLE => {
            "foundationdb_coordinators_unavailable"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_DATABASE_NOT_CONFIGURED => {
            "foundationdb_database_not_configured"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_DATABASE_UNAVAILABLE => {
            "foundationdb_database_unavailable"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_CONFIGURATION_INVALID => {
            "foundationdb_configuration_invalid"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_STALE_CLUSTER_FILE => {
            "foundationdb_stale_cluster_file"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_RECRUITMENT_PENDING => {
            "foundationdb_recruitment_pending"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_RECOVERY_IN_PROGRESS => {
            "foundationdb_recovery_in_progress"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_DATA_MOVEMENT_IN_PROGRESS => {
            "foundationdb_data_movement_in_progress"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_COORDINATOR_QUORUM_LOST => {
            "foundationdb_coordinator_quorum_lost"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_INSUFFICIENT_STORAGE_FOR_REDUNDANCY => {
            "foundationdb_insufficient_storage_for_redundancy"
        }
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_UNSPECIFIED => "unspecified",
    }
}

#[cfg(unix)]
async fn set_audit_permissions(path: &Path) -> AgentResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = std::fs::Permissions::from_mode(0o600);
    tokio::fs::set_permissions(path, permissions)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
}

#[cfg(not(unix))]
async fn set_audit_permissions(_path: &Path) -> AgentResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests;
