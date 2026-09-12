// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Mutex;
use std::time::Duration;

use buffa::MessageField;
use secrecy::SecretString;

use crate::config::HephaestusAgentConfig;
use crate::docker_service::{DockerServiceSecretResolver, SecretRef};
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

use super::{
    AgentActionControlPlane, AgentActionOutcome, AgentActionReceipt, ExecutableAgentAction,
    agent_action_execution_timeout, execute_polled_actions, foundationdb_action_result_reason, pb,
};

struct FakeActionControlPlane {
    completions: Mutex<Vec<String>>,
    fail_next_completion: Mutex<bool>,
    runtime_authority: Option<pb::DockerRuntimeAuthority>,
    authority_requests: Mutex<Vec<String>>,
}

impl FakeActionControlPlane {
    fn new_fail_first_completion() -> Self {
        Self {
            completions: Mutex::new(Vec::new()),
            fail_next_completion: Mutex::new(true),
            runtime_authority: None,
            authority_requests: Mutex::new(Vec::new()),
        }
    }

    fn with_runtime_authority(authority: pb::DockerRuntimeAuthority) -> Self {
        Self {
            completions: Mutex::new(Vec::new()),
            fail_next_completion: Mutex::new(false),
            runtime_authority: Some(authority),
            authority_requests: Mutex::new(Vec::new()),
        }
    }

    fn completed_action_ids(&self) -> Vec<String> {
        self.completions
            .lock()
            .expect("test completion lock should not be poisoned")
            .clone()
    }

    fn authority_service_ids(&self) -> Vec<String> {
        self.authority_requests
            .lock()
            .expect("test authority request lock should not be poisoned")
            .clone()
    }
}

impl AgentActionControlPlane for FakeActionControlPlane {
    fn resolve_docker_runtime_authority<'a>(
        &'a self,
        _receipt: &'a AgentActionReceipt,
        service_id: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = AgentResult<Option<pb::DockerRuntimeAuthority>>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.authority_requests
                .lock()
                .expect("test authority request lock should not be poisoned")
                .push(service_id.to_owned());
            Ok(self.runtime_authority.clone())
        })
    }

    fn submit_agent_report<'a>(
        &'a self,
        _report: &'a reallyme_hephaestus_domain::HephaestusAgentReport,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AgentResult<()>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }

    fn complete_agent_action<'a>(
        &'a self,
        receipt: &'a AgentActionReceipt,
        _outcome: AgentActionOutcome,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AgentResult<()>> + Send + 'a>> {
        Box::pin(async move {
            self.completions
                .lock()
                .expect("test completion lock should not be poisoned")
                .push(receipt.action_id().to_owned());
            let mut fail_next = self
                .fail_next_completion
                .lock()
                .expect("test failure lock should not be poisoned");
            if *fail_next {
                *fail_next = false;
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::ConnectFailed,
                ));
            }
            Ok(())
        })
    }
}

#[test]
fn action_execution_timeout_scales_with_command_timeout() {
    assert_eq!(
        agent_action_execution_timeout(Duration::from_secs(15)),
        Duration::from_secs(60)
    );
}

impl DockerServiceSecretResolver for FakeActionControlPlane {
    fn resolve_agent_secret<'a>(
        &'a self,
        _receipt: &'a AgentActionReceipt,
        secret_ref: &'a SecretRef,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AgentResult<SecretString>> + Send + 'a>>
    {
        let secret_ref = secret_ref.as_str().to_owned();
        Box::pin(async move {
            let resolved = match secret_ref.as_str() {
                "services/nats/auth-token" => "nats-token",
                "services/nats/conn-string" => "nats-conn",
                _ => "test-secret",
            };
            Ok(SecretString::new(resolved.to_owned().into()))
        })
    }
}

fn valid_action(kind: pb::AgentActionKind) -> pb::AgentAction {
    pb::AgentAction {
        action_id: "act-1".to_owned(),
        node_id: "prod-nats-regional-ams-01".to_owned(),
        desired_generation: 7,
        deadline_unix_secs: 2_000,
        idempotency_key: "idem-1".to_owned(),
        kind: kind.into(),
        ..Default::default()
    }
}

#[test]
fn accepts_report_now_for_matching_node() {
    let action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_REPORT_NOW);

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert!(result.is_ok());
}

#[test]
fn rejects_wrong_node() {
    let action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_REPORT_NOW);

    let result = ExecutableAgentAction::from_proto(action, "other-node", 1_000);

    assert_eq!(
        result.err(),
        Some(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_WRONG_NODE
        ))
    );
}

#[test]
fn rejects_expired_action() {
    let action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_REPORT_NOW);

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 2_001);

    assert_eq!(
        result.err(),
        Some(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_EXPIRED
        ))
    );
}

#[test]
fn requires_docker_payload_for_pull_image() {
    let action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_PULL_IMAGE);

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert_eq!(
        result.err(),
        Some(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION
        ))
    );
}

#[test]
fn accepts_docker_payload_for_pull_image() {
    let mut action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_PULL_IMAGE);
    action.docker = MessageField::some(pb::AgentDockerAction {
        image_ref: "ams.vultrcr.com/reallyme/nats:2.14.0-amd64".to_owned(),
        ..Default::default()
    });

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert!(result.is_ok());
}

#[test]
fn rejects_shell_metacharacters_in_image_ref() {
    let mut action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_PULL_IMAGE);
    action.docker = MessageField::some(pb::AgentDockerAction {
        image_ref: "ams.vultrcr.com/reallyme/nats:latest;shutdown".to_owned(),
        ..Default::default()
    });

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert_eq!(
        result.err(),
        Some(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION
        ))
    );
}

#[test]
fn rejects_unsupported_equals_character_in_pull_image_ref() {
    let mut action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_PULL_IMAGE);
    action.docker = MessageField::some(pb::AgentDockerAction {
        image_ref: "ams.vultrcr.com/reallyme/nats=latest".to_owned(),
        ..Default::default()
    });

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert_eq!(
        result.err(),
        Some(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION
        ))
    );
}

#[test]
fn accepts_container_restart_payload() {
    let mut action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_RESTART_CONTAINER);
    action.docker = MessageField::some(pb::AgentDockerAction {
        container_name: "reallyme-nats".to_owned(),
        ..Default::default()
    });

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert!(result.is_ok());
}

#[test]
fn accepts_foundationdb_service_configure_payload() {
    let mut action =
        valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_CONFIGURE_FOUNDATIONDB_SERVICE);
    action.foundationdb_service = MessageField::some(pb::AgentFoundationDbServiceAction {
        cluster_description: "fdb_staging_hel".to_owned(),
        cluster_token: "0123456789abcdef0123456789abcdef".to_owned(),
        coordinators: vec![pb::AgentFoundationDbCoordinator {
            address: "100.83.14.29".to_owned(),
            port: 4500,
            ..Default::default()
        }],
        public_address: "100.83.14.29".to_owned(),
        listen_port: 4500,
        datacenter_id: "hel".to_owned(),
        zone_id: "hel-01".to_owned(),
        machine_id: "hetzner-staging-fdb-hel-01".to_owned(),
        process_class: "storage".to_owned(),
        redundancy_mode: "single".to_owned(),
        storage_engine: "ssd-2".to_owned(),
        initialize_database: true,
        cluster_operation: pb::AgentFoundationDbClusterOperation::AGENT_FOUNDATION_DB_CLUSTER_OPERATION_CREATE_CLUSTER
            .into(),
        ..Default::default()
    });

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert!(result.is_ok());
}

#[test]
fn rejects_foundationdb_service_payload_with_shell_address() {
    let mut action =
        valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_CONFIGURE_FOUNDATIONDB_SERVICE);
    action.foundationdb_service = MessageField::some(pb::AgentFoundationDbServiceAction {
        cluster_description: "fdb_staging_hel".to_owned(),
        cluster_token: "0123456789abcdef0123456789abcdef".to_owned(),
        coordinators: vec![pb::AgentFoundationDbCoordinator {
            address: "100.83.14.29;reboot".to_owned(),
            port: 4500,
            ..Default::default()
        }],
        public_address: "100.83.14.29".to_owned(),
        listen_port: 4500,
        datacenter_id: "hel".to_owned(),
        zone_id: "hel-01".to_owned(),
        machine_id: "hetzner-staging-fdb-hel-01".to_owned(),
        process_class: "storage".to_owned(),
        redundancy_mode: "single".to_owned(),
        storage_engine: "ssd-2".to_owned(),
        initialize_database: true,
        cluster_operation: pb::AgentFoundationDbClusterOperation::AGENT_FOUNDATION_DB_CLUSTER_OPERATION_CREATE_CLUSTER
            .into(),
        ..Default::default()
    });

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert_eq!(
        result.err(),
        Some(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION
        ))
    );
}

#[test]
fn maps_foundationdb_configure_failure_to_typed_completion_reason() {
    let reason = foundationdb_action_result_reason(
        crate::foundationdb_service::FoundationDbServiceError::ConfigureNewFailed,
    );

    assert_eq!(
        reason,
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_CONFIGURE_NEW_FAILED
    );
}

#[test]
fn maps_foundationdb_status_failure_to_typed_completion_reason() {
    let reason = foundationdb_action_result_reason(
        crate::foundationdb_service::FoundationDbServiceError::StatusUnavailable,
    );

    assert_eq!(
        reason,
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_STATUS_UNAVAILABLE
    );
}

#[test]
fn maps_foundationdb_coordinator_failure_to_typed_completion_reason() {
    let reason = foundationdb_action_result_reason(
        crate::foundationdb_service::FoundationDbServiceError::CoordinatorsUnavailable,
    );

    assert_eq!(
        reason,
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_COORDINATORS_UNAVAILABLE
    );
}

#[test]
fn maps_foundationdb_not_configured_to_typed_completion_reason() {
    let reason = foundationdb_action_result_reason(
        crate::foundationdb_service::FoundationDbServiceError::DatabaseNotConfigured,
    );

    assert_eq!(
        reason,
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_DATABASE_NOT_CONFIGURED
    );
}

#[test]
fn maps_foundationdb_unavailable_to_typed_completion_reason() {
    let reason = foundationdb_action_result_reason(
        crate::foundationdb_service::FoundationDbServiceError::DatabaseUnavailable,
    );

    assert_eq!(
        reason,
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_DATABASE_UNAVAILABLE
    );
}

#[test]
fn maps_foundationdb_invalid_configuration_to_typed_completion_reason() {
    let reason = foundationdb_action_result_reason(
        crate::foundationdb_service::FoundationDbServiceError::ConfigurationInvalid,
    );

    assert_eq!(
        reason,
        pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_CONFIGURATION_INVALID
    );
}

#[test]
fn maps_foundationdb_observation_failures_to_typed_completion_reasons() {
    let cases = [
        (
            crate::foundationdb_service::FoundationDbServiceError::StaleClusterFile,
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_STALE_CLUSTER_FILE,
        ),
        (
            crate::foundationdb_service::FoundationDbServiceError::RecruitmentPending,
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_RECRUITMENT_PENDING,
        ),
        (
            crate::foundationdb_service::FoundationDbServiceError::RecoveryInProgress,
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_RECOVERY_IN_PROGRESS,
        ),
        (
            crate::foundationdb_service::FoundationDbServiceError::DataMovementInProgress,
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_DATA_MOVEMENT_IN_PROGRESS,
        ),
        (
            crate::foundationdb_service::FoundationDbServiceError::CoordinatorQuorumLost,
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_COORDINATOR_QUORUM_LOST,
        ),
        (
            crate::foundationdb_service::FoundationDbServiceError::InsufficientStorageForRedundancy,
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_FOUNDATIONDB_INSUFFICIENT_STORAGE_FOR_REDUNDANCY,
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(foundationdb_action_result_reason(error), expected);
    }
}

#[test]
fn rejects_container_name_with_path_separator() {
    let mut action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_STOP_CONTAINER);
    action.docker = MessageField::some(pb::AgentDockerAction {
        container_name: "../reallyme-nats".to_owned(),
        ..Default::default()
    });

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert_eq!(
        result.err(),
        Some(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION
        ))
    );
}

#[test]
fn accepts_configure_docker_service_payload() {
    let mut action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_CONFIGURE_DOCKER_SERVICE);
    action.docker_service = MessageField::some(pb::AgentDockerServiceAction {
        service_id: "nats".to_owned(),
        container_name: "reallyme-nats".to_owned(),
        image_ref: "ams.vultrcr.com/reallyme/nats:2.14.0-amd64".to_owned(),
        restart_policy: pb::AgentDockerRestartPolicy::AGENT_DOCKER_RESTART_POLICY_UNLESS_STOPPED
            .into(),
        ports: vec![pb::AgentDockerPort {
            protocol: pb::AgentDockerPortProtocol::AGENT_DOCKER_PORT_PROTOCOL_TCP.into(),
            host_ip: "127.0.0.1".to_owned(),
            host_port: 4222,
            container_port: 4222,
            ..Default::default()
        }],
        volumes: vec![pb::AgentDockerVolume {
            host_path: "/var/lib/reallyme/nats/data".to_owned(),
            container_path: "/data".to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    });

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert!(result.is_ok());
}

#[tokio::test]
async fn resolves_runtime_authority_for_validated_docker_service_action() {
    let mut action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_CONFIGURE_DOCKER_SERVICE);
    action.docker_service = MessageField::some(pb::AgentDockerServiceAction {
        service_id: "network-edge".to_owned(),
        container_name: "network-edge".to_owned(),
        image_ref: "registry.example.invalid/network-edge@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        restart_policy: pb::AgentDockerRestartPolicy::AGENT_DOCKER_RESTART_POLICY_UNLESS_STOPPED
            .into(),
        ..Default::default()
    });
    let authority = pb::DockerRuntimeAuthority {
        schema_version: 1,
        devices: vec![pb::DockerHostDevice::DOCKER_HOST_DEVICE_TUN.into()],
        linux_capabilities: vec![
            pb::DockerLinuxCapability::DOCKER_LINUX_CAPABILITY_NET_ADMIN.into(),
        ],
        read_only_root_filesystem: true,
        no_new_privileges: true,
        network_mode: pb::DockerNetworkMode::DOCKER_NETWORK_MODE_HOST.into(),
        __buffa_unknown_fields: Default::default(),
    };
    let client = FakeActionControlPlane::with_runtime_authority(authority);
    let mut executable =
        ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000)
            .expect("valid docker service action");

    let result = executable.apply_docker_runtime_authority(&client).await;

    assert!(result.is_ok());
    assert_eq!(
        client.authority_service_ids(),
        vec![String::from("network-edge")]
    );
}

#[test]
fn rejects_configure_docker_service_with_unsafe_volume() {
    let mut action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_CONFIGURE_DOCKER_SERVICE);
    action.docker_service = MessageField::some(pb::AgentDockerServiceAction {
        service_id: "nats".to_owned(),
        container_name: "reallyme-nats".to_owned(),
        image_ref: "ams.vultrcr.com/reallyme/nats:2.14.0-amd64".to_owned(),
        restart_policy: pb::AgentDockerRestartPolicy::AGENT_DOCKER_RESTART_POLICY_UNLESS_STOPPED
            .into(),
        volumes: vec![pb::AgentDockerVolume {
            host_path: "/tmp/nats".to_owned(),
            container_path: "/data".to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    });

    let result = ExecutableAgentAction::from_proto(action, "prod-nats-regional-ams-01", 1_000);

    assert_eq!(
        result.err(),
        Some(AgentActionOutcome::rejected(
            pb::AgentActionResultReason::AGENT_ACTION_RESULT_REASON_INVALID_ACTION
        ))
    );
}

#[test]
fn malformed_action_without_reportable_receipt_still_has_replay_guard_generation() {
    let mut action = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_REPORT_NOW);
    action.action_id = "../unsafe".to_owned();

    assert!(super::rejected_receipt_from_proto(&action, "prod-nats-regional-ams-01").is_none());
    assert_eq!(super::replay_guard_generation_from_action(&action), 7);
}

#[tokio::test]
async fn execute_polled_actions_continues_after_completion_report_failure() {
    let config = test_config("partial-completion-failure")
        .await
        .expect("test config should load");
    let client = FakeActionControlPlane::new_fail_first_completion();
    let mut first = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_REPORT_NOW);
    first.node_id = "wrong-node".to_owned();
    let mut second = valid_action(pb::AgentActionKind::AGENT_ACTION_KIND_REPORT_NOW);
    second.action_id = "act-2".to_owned();
    second.idempotency_key = "idem-2".to_owned();
    second.node_id = "wrong-node".to_owned();
    second.desired_generation = 8;

    let observed_generation = execute_polled_actions(&config, &client, vec![first, second])
        .await
        .expect("batch should continue after completion reporting failure");

    assert_eq!(observed_generation, 8);
    assert_eq!(
        client.completed_action_ids(),
        vec![String::from("act-1"), String::from("act-2")]
    );
    let audit_log = tokio::fs::read_to_string(config.audit_log_path())
        .await
        .expect("audit log should be readable");
    assert!(audit_log.contains("completion_report_failed"));
    assert!(audit_log.contains("completion_reported"));
}

async fn test_config(name: &str) -> AgentResult<HephaestusAgentConfig> {
    let base = std::env::temp_dir().join(format!("hephaestus-agent-{name}-{}", std::process::id()));
    tokio::fs::create_dir_all(base.as_path())
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    let config_path = base.join("config.toml");
    let audit_path = base.join("action-audit.jsonl");
    let state_dir = base.join("state");
    let bootstrap_path = base.join("bootstrap-token");
    let document = format!(
        r#"
controller_base_url = "https://heph.example.ts.net"
server_id = "prod-nats-regional-ams-01"
provider_server_id = "vultr-123"
site_id = "ams"
bootstrap_token_path = "{}"
state_dir = "{}"
audit_log_path = "{}"
full_report_interval_seconds = 30
action_poll_interval_seconds = 2
command_timeout_seconds = 1
"#,
        bootstrap_path.display(),
        state_dir.display(),
        audit_path.display()
    );
    tokio::fs::write(config_path.as_path(), document)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    HephaestusAgentConfig::load(config_path.as_path()).await
}
