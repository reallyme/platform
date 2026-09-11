// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{AgentActionAuditLog, AgentAuditActionMetadata};
use crate::actions::AgentActionOutcome;
use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1 as pb;

fn sample_action() -> pb::AgentAction {
    pb::AgentAction {
        action_id: "act-1".to_owned(),
        node_id: "prod-nats-regional-ams-01".to_owned(),
        desired_generation: 7,
        deadline_unix_secs: 2_000,
        idempotency_key: "idem-1".to_owned(),
        kind: pb::AgentActionKind::AGENT_ACTION_KIND_CONFIGURE_DOCKER_SERVICE.into(),
        docker_service: buffa::MessageField::some(pb::AgentDockerServiceAction {
            service_id: "nats".to_owned(),
            image_ref: "registry.invalid/reallyme/nats:latest".to_owned(),
            secret_files: vec![pb::AgentDockerSecretFile {
                relative_path: "auth/token".to_owned(),
                secret_ref: "services/nats/auth-token".to_owned(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[tokio::test]
async fn writes_redacted_jsonl_action_records() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error:?}"));
    let path = dir.path().join("action-audit.jsonl");
    let log = AgentActionAuditLog::new(path.as_path())
        .unwrap_or_else(|error| panic!("audit log: {error:?}"));
    let metadata =
        AgentAuditActionMetadata::from_action(&sample_action(), "prod-nats-regional-ams-01");

    log.record_received(&metadata)
        .await
        .unwrap_or_else(|error| panic!("audit write: {error:?}"));
    log.record_completed(&metadata, AgentActionOutcome::succeeded(false))
        .await
        .unwrap_or_else(|error| panic!("audit write: {error:?}"));

    let contents = tokio::fs::read_to_string(path.as_path())
        .await
        .unwrap_or_else(|error| panic!("audit read: {error:?}"));
    assert!(contents.contains("\"event\":\"received\""));
    assert!(contents.contains("\"event\":\"completed\""));
    assert!(contents.contains("\"action_kind\":\"configure_docker_service\""));
    assert!(!contents.contains("auth-token"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = tokio::fs::metadata(path.as_path())
            .await
            .unwrap_or_else(|error| panic!("metadata: {error:?}"));
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn redacts_unsafe_action_identifiers() {
    let mut action = sample_action();
    action.action_id = "bad/action".to_owned();
    let metadata = AgentAuditActionMetadata::from_action(&action, "prod-nats-regional-ams-01");

    assert_eq!(metadata.action_id, None);
    assert!(metadata.target_node_matches);
}
