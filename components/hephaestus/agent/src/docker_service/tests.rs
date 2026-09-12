// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::file_ops::{copy_dir_recursive, rollback_dir, write_secret_file_atomic};
use super::validation::validate_host_path_on_disk;
use super::{
    DockerServiceSpec, HealthProbe, HealthProbeKind, ResolvedSecretFile, health_gate,
    parse_numeric_container_user, render_compose, repo_digests_contain_digest,
};
use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1 as pb;
use secrecy::SecretString;
use serde_norway::Value;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tokio::time::{Duration, sleep};

fn sample_action() -> pb::AgentDockerServiceAction {
    pb::AgentDockerServiceAction {
        service_id: "nats".to_owned(),
        container_name: "reallyme-nats".to_owned(),
        image_ref: "ams.vultrcr.com/reallyme/nats:2.14.0-amd64".to_owned(),
        restart_policy: pb::AgentDockerRestartPolicy::AGENT_DOCKER_RESTART_POLICY_UNLESS_STOPPED
            .into(),
        environment: vec![pb::AgentDockerEnvironmentVariable {
            name: "NATS_SERVER_NAME".to_owned(),
            value: "prod-nats-regional-ams-01".to_owned(),
            sensitive: false,
            ..Default::default()
        }],
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
            read_only: false,
            ..Default::default()
        }],
        health_probes: vec![pb::AgentDockerHealthProbe {
            name: "nats-health".to_owned(),
            kind: pb::AgentDockerHealthProbeKind::AGENT_DOCKER_HEALTH_PROBE_KIND_HTTP.into(),
            target: "http://127.0.0.1:8222/healthz".to_owned(),
            expected_http_status: 200,
            ..Default::default()
        }],
        tailscale_services: vec![pb::AgentTailscaleServiceConfig {
            service_name: "svc:nats-eu".to_owned(),
            endpoints: vec![pb::AgentTailscaleServiceEndpoint {
                listen: "tcp:4222".to_owned(),
                upstream: "tcp://127.0.0.1:4222".to_owned(),
                ..Default::default()
            }],
            ..Default::default()
        }],
        config_files: vec![pb::AgentDockerRenderedFile {
            relative_path: "nats.conf".to_owned(),
            contents: "server_name: prod-nats-regional-ams-01\n".to_owned(),
            executable: false,
            ..Default::default()
        }],
        secret_files: vec![pb::AgentDockerSecretFile {
            relative_path: "auth/token".to_owned(),
            secret_ref: "services/nats/auth-token".to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn tun_runtime_authority() -> pb::DockerRuntimeAuthority {
    pb::DockerRuntimeAuthority {
        schema_version: 1,
        devices: vec![pb::DockerHostDevice::DOCKER_HOST_DEVICE_TUN.into()],
        linux_capabilities: vec![
            pb::DockerLinuxCapability::DOCKER_LINUX_CAPABILITY_NET_BIND_SERVICE.into(),
            pb::DockerLinuxCapability::DOCKER_LINUX_CAPABILITY_NET_ADMIN.into(),
        ],
        read_only_root_filesystem: true,
        no_new_privileges: true,
        network_mode: pb::DockerNetworkMode::DOCKER_NETWORK_MODE_HOST.into(),
        __buffa_unknown_fields: Default::default(),
    }
}

#[test]
fn renders_deterministic_compose_with_environment_block() {
    let spec = DockerServiceSpec::from_proto(sample_action())
        .unwrap_or_else(|error| panic!("valid docker service: {error:?}"));

    let compose =
        render_compose(&spec).unwrap_or_else(|error| panic!("compose rendered: {error:?}"));

    assert!(compose.contains("ams.vultrcr.com/reallyme/nats:2.14.0-amd64"));
    assert!(compose.contains("environment:"));
    assert!(compose.contains("NATS_SERVER_NAME: prod-nats-regional-ams-01"));
    assert!(compose.contains("127.0.0.1:4222:4222/tcp"));
    assert!(compose.contains("/etc/reallyme/services/nats/config:/config:ro"));
    assert!(compose.contains("/etc/reallyme/secrets/nats:/run/secrets/reallyme:ro"));
}

#[test]
fn renders_tun_authority_without_privileged_container_access() {
    let mut action = sample_action();
    action.service_id = "network-edge".to_owned();
    action.container_name = "network-edge".to_owned();
    action.volumes = Vec::new();
    let authority = tun_runtime_authority();

    let spec = DockerServiceSpec::from_proto_with_runtime_authority(action, Some(authority))
        .unwrap_or_else(|error| panic!("valid TUN service: {error:?}"));
    let compose =
        render_compose(&spec).unwrap_or_else(|error| panic!("compose rendered: {error:?}"));
    let parsed = serde_norway::from_str::<Value>(compose.as_str())
        .unwrap_or_else(|error| panic!("compose parse: {error:?}"));
    let service = parsed
        .get("services")
        .and_then(Value::as_mapping)
        .and_then(|services| services.get(Value::String("network-edge".to_owned())))
        .and_then(Value::as_mapping)
        .unwrap_or_else(|| panic!("compose has network-edge service"));

    assert_eq!(
        service
            .get(Value::String("devices".to_owned()))
            .and_then(Value::as_sequence),
        Some(&vec![Value::String(
            "/dev/net/tun:/dev/net/tun:rwm".to_owned()
        )])
    );
    assert_eq!(
        service
            .get(Value::String("network_mode".to_owned()))
            .and_then(Value::as_str),
        Some("host")
    );
    assert!(
        service
            .get(Value::String("ports".to_owned()))
            .and_then(Value::as_sequence)
            .is_none_or(Vec::is_empty)
    );
    assert_eq!(
        service
            .get(Value::String("read_only".to_owned()))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        service
            .get(Value::String("cap_add".to_owned()))
            .and_then(Value::as_sequence),
        Some(&vec![
            Value::String("NET_ADMIN".to_owned()),
            Value::String("NET_BIND_SERVICE".to_owned()),
        ])
    );
    assert_eq!(
        service
            .get(Value::String("cap_drop".to_owned()))
            .and_then(Value::as_sequence),
        Some(&vec![Value::String("ALL".to_owned())])
    );
    assert_eq!(
        service
            .get(Value::String("security_opt".to_owned()))
            .and_then(Value::as_sequence),
        Some(&vec![Value::String("no-new-privileges:true".to_owned())])
    );
    assert!(!service.contains_key(Value::String("privileged".to_owned())));
}

#[test]
fn rejects_tun_device_without_net_admin() {
    let mut authority = tun_runtime_authority();
    authority.linux_capabilities = Vec::new();

    assert!(
        DockerServiceSpec::from_proto_with_runtime_authority(sample_action(), Some(authority))
            .is_err()
    );
}

#[test]
fn rejects_duplicate_runtime_authority() {
    let mut authority = tun_runtime_authority();
    authority
        .devices
        .push(pb::DockerHostDevice::DOCKER_HOST_DEVICE_TUN.into());

    assert!(
        DockerServiceSpec::from_proto_with_runtime_authority(sample_action(), Some(authority))
            .is_err()
    );
}

#[test]
fn rejects_unknown_runtime_authority_values() {
    let mut authority = tun_runtime_authority();
    authority.devices = vec![buffa::EnumValue::from(99_i32)];

    assert!(
        DockerServiceSpec::from_proto_with_runtime_authority(sample_action(), Some(authority))
            .is_err()
    );
}

#[test]
fn rejects_unsupported_runtime_authority_schema() {
    let mut authority = tun_runtime_authority();
    authority.schema_version = 2;

    assert!(
        DockerServiceSpec::from_proto_with_runtime_authority(sample_action(), Some(authority))
            .is_err()
    );
}

#[test]
fn rejects_unspecified_runtime_network_mode() {
    let mut authority = tun_runtime_authority();
    authority.network_mode = pb::DockerNetworkMode::DOCKER_NETWORK_MODE_UNSPECIFIED.into();

    assert!(
        DockerServiceSpec::from_proto_with_runtime_authority(sample_action(), Some(authority))
            .is_err()
    );
}

#[test]
fn preserves_boolean_like_container_name_in_rendered_compose() {
    let mut action = sample_action();
    action.container_name = "no".to_owned();

    let spec = DockerServiceSpec::from_proto(action)
        .unwrap_or_else(|error| panic!("valid docker service: {error:?}"));
    let compose =
        render_compose(&spec).unwrap_or_else(|error| panic!("compose rendered: {error:?}"));

    let parsed = serde_norway::from_str::<Value>(compose.as_str())
        .unwrap_or_else(|error| panic!("compose parse: {error:?}"));
    let services = parsed
        .get("services")
        .and_then(Value::as_mapping)
        .unwrap_or_else(|| panic!("compose has services map"));
    let service = services
        .get(Value::String("nats".to_owned()))
        .and_then(Value::as_mapping)
        .unwrap_or_else(|| panic!("compose has nats service"));
    let container_name = service
        .get(Value::String("container_name".to_owned()))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("compose has container_name"));

    assert_eq!(container_name, "no");
}

#[test]
fn renders_production_nats_healthcheck_with_jetstream_gate() {
    let mut action = sample_action();
    action.service_id = "reallyme-nats".to_owned();
    action.volumes[0].host_path = "/var/lib/reallyme/nats".to_owned();
    action.volumes[0].container_path = "/data/nats".to_owned();
    let spec = DockerServiceSpec::from_proto(action)
        .unwrap_or_else(|error| panic!("valid docker service: {error:?}"));

    let compose =
        render_compose(&spec).unwrap_or_else(|error| panic!("compose rendered: {error:?}"));
    let parsed = serde_norway::from_str::<Value>(compose.as_str())
        .unwrap_or_else(|error| panic!("compose parse: {error:?}"));
    let services = parsed
        .get("services")
        .and_then(Value::as_mapping)
        .unwrap_or_else(|| panic!("compose has services map"));
    let service = services
        .get(Value::String("nats".to_owned()))
        .and_then(Value::as_mapping)
        .unwrap_or_else(|| panic!("compose has nats service"));
    let healthcheck = service
        .get(Value::String("healthcheck".to_owned()))
        .and_then(Value::as_mapping)
        .unwrap_or_else(|| panic!("compose has healthcheck"));
    let test = healthcheck
        .get(Value::String("test".to_owned()))
        .and_then(Value::as_sequence)
        .unwrap_or_else(|| panic!("healthcheck has test sequence"));

    assert_eq!(
        test.get(1).and_then(Value::as_str),
        Some("wget -q -O /dev/null 'http://127.0.0.1:8222/healthz?js-enabled-only=true'")
    );
}

#[test]
fn renders_production_web_service_with_host_network_and_hardening() {
    let mut action = sample_action();
    action.service_id = "reallyme-web".to_owned();
    action.container_name = "reallyme-web".to_owned();
    action.image_ref = "ghcr.io/reallyme/reallyme-web:staging".to_owned();
    action.environment = vec![pb::AgentDockerEnvironmentVariable {
        name: "PORT".to_owned(),
        value: "3000".to_owned(),
        sensitive: false,
        ..Default::default()
    }];
    action.ports = vec![pb::AgentDockerPort {
        protocol: pb::AgentDockerPortProtocol::AGENT_DOCKER_PORT_PROTOCOL_TCP.into(),
        host_ip: "127.0.0.1".to_owned(),
        host_port: 3000,
        container_port: 3000,
        ..Default::default()
    }];
    action.volumes = Vec::new();
    action.config_files = Vec::new();
    action.secret_files = Vec::new();
    action.health_probes = vec![pb::AgentDockerHealthProbe {
        name: "web-health".to_owned(),
        kind: pb::AgentDockerHealthProbeKind::AGENT_DOCKER_HEALTH_PROBE_KIND_HTTP.into(),
        target: "http://127.0.0.1:3000/".to_owned(),
        expected_http_status: 200,
        ..Default::default()
    }];

    let spec = DockerServiceSpec::from_proto(action)
        .unwrap_or_else(|error| panic!("valid web docker service: {error:?}"));
    let compose =
        render_compose(&spec).unwrap_or_else(|error| panic!("compose rendered: {error:?}"));
    let parsed = serde_norway::from_str::<Value>(compose.as_str())
        .unwrap_or_else(|error| panic!("compose parse: {error:?}"));
    let service = parsed
        .get("services")
        .and_then(Value::as_mapping)
        .and_then(|services| services.get(Value::String("reallyme-web".to_owned())))
        .and_then(Value::as_mapping)
        .unwrap_or_else(|| panic!("compose has reallyme-web service"));

    assert_eq!(
        service
            .get(Value::String("network_mode".to_owned()))
            .and_then(Value::as_str),
        Some("host")
    );
    assert_eq!(
        service
            .get(Value::String("read_only".to_owned()))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        service
            .get(Value::String("cap_drop".to_owned()))
            .and_then(Value::as_sequence)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("ALL")))
    );
    assert!(
        service
            .get(Value::String("ports".to_owned()))
            .and_then(Value::as_sequence)
            .is_none_or(Vec::is_empty)
    );
}

#[test]
fn renders_production_caddy_service_for_host_local_web_upstream() {
    let mut action = sample_action();
    action.service_id = "reallyme-caddy".to_owned();
    action.container_name = "reallyme-caddy".to_owned();
    action.image_ref = "ams.vultrcr.com/reallyme/caddy:2.11.3-amd64".to_owned();
    action.environment = vec![
        pb::AgentDockerEnvironmentVariable {
            name: "CADDY_UPSTREAM".to_owned(),
            value: "http://127.0.0.1:3000".to_owned(),
            sensitive: false,
            ..Default::default()
        },
        pb::AgentDockerEnvironmentVariable {
            name: "CADDY_HTTP_PORT".to_owned(),
            value: "8080".to_owned(),
            sensitive: false,
            ..Default::default()
        },
    ];
    action.ports = vec![pb::AgentDockerPort {
        protocol: pb::AgentDockerPortProtocol::AGENT_DOCKER_PORT_PROTOCOL_TCP.into(),
        host_ip: "0.0.0.0".to_owned(),
        host_port: 8080,
        container_port: 8080,
        ..Default::default()
    }];
    action.volumes = vec![pb::AgentDockerVolume {
        host_path: "/var/lib/reallyme/reallyme-caddy/data".to_owned(),
        container_path: "/data/caddy".to_owned(),
        read_only: false,
        ..Default::default()
    }];
    action.config_files = Vec::new();
    action.secret_files = Vec::new();
    action.health_probes = vec![pb::AgentDockerHealthProbe {
        name: "caddy-health".to_owned(),
        kind: pb::AgentDockerHealthProbeKind::AGENT_DOCKER_HEALTH_PROBE_KIND_HTTP.into(),
        target: "http://127.0.0.1:8080/healthz".to_owned(),
        expected_http_status: 200,
        ..Default::default()
    }];

    let spec = DockerServiceSpec::from_proto(action)
        .unwrap_or_else(|error| panic!("valid caddy docker service: {error:?}"));
    let compose =
        render_compose(&spec).unwrap_or_else(|error| panic!("compose rendered: {error:?}"));
    let parsed = serde_norway::from_str::<Value>(compose.as_str())
        .unwrap_or_else(|error| panic!("compose parse: {error:?}"));
    let service = parsed
        .get("services")
        .and_then(Value::as_mapping)
        .and_then(|services| services.get(Value::String("reallyme-caddy".to_owned())))
        .and_then(Value::as_mapping)
        .unwrap_or_else(|| panic!("compose has reallyme-caddy service"));

    assert_eq!(
        service
            .get(Value::String("network_mode".to_owned()))
            .and_then(Value::as_str),
        Some("host")
    );
    assert_eq!(
        service
            .get(Value::String("read_only".to_owned()))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        service
            .get(Value::String("cap_drop".to_owned()))
            .and_then(Value::as_sequence)
            .is_none_or(Vec::is_empty)
    );
    assert!(
        service
            .get(Value::String("tmpfs".to_owned()))
            .and_then(Value::as_sequence)
            .is_some_and(|values| values.iter().any(|value| value
                .as_str()
                .is_some_and(|item| item.starts_with("/run/caddy:"))))
    );
}

#[test]
fn validates_expected_repo_digest() {
    let digests = vec![
            "registry.invalid/reallyme/nats@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
        ];

    assert!(repo_digests_contain_digest(
        digests.as_slice(),
        "registry.invalid/reallyme/nats:2.14.0",
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    ));
    assert!(!repo_digests_contain_digest(
        digests.as_slice(),
        "registry.invalid/reallyme/typesense:28.0",
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    ));
    assert!(!repo_digests_contain_digest(
        digests.as_slice(),
        "registry.invalid/reallyme/nats:2.14.0",
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    ));
}

#[test]
fn parses_numeric_container_user_for_volume_ownership() {
    assert_eq!(
        parse_numeric_container_user("1000:1000")
            .unwrap_or_else(|error| panic!("numeric user parses: {error:?}")),
        Some((String::from("1000"), String::from("1000")))
    );
    assert_eq!(
        parse_numeric_container_user("0:0")
            .unwrap_or_else(|error| panic!("root user parses: {error:?}")),
        None
    );
    assert!(parse_numeric_container_user("nats").is_err());
}

#[test]
fn rollback_state_uses_agent_owned_directory() {
    let path = rollback_dir("reallyme-typesense");

    assert_eq!(
        path,
        PathBuf::from("/var/lib/reallyme/hephaestus-agent/rollback/reallyme-typesense")
    );
    assert!(!path.starts_with("/var/lib/reallyme/typesense"));
}

#[test]
fn rejects_host_path_outside_service_roots() {
    let mut action = sample_action();
    action.volumes[0].host_path = "/tmp/nats".to_owned();

    let result = DockerServiceSpec::from_proto(action);

    assert!(result.is_err());
}

#[test]
fn accepts_production_nats_data_root_for_reallyme_service_id() {
    let mut action = sample_action();
    action.service_id = "reallyme-nats".to_owned();
    action.volumes[0].host_path = "/var/lib/reallyme/nats".to_owned();
    action.volumes[0].container_path = "/data/nats".to_owned();

    let result = DockerServiceSpec::from_proto(action);

    assert!(result.is_ok());
}

#[test]
fn accepts_production_typesense_data_root_for_reallyme_service_id() {
    let mut action = sample_action();
    action.service_id = "reallyme-typesense".to_owned();
    action.container_name = "reallyme-typesense".to_owned();
    action.volumes[0].host_path = "/var/lib/reallyme/typesense".to_owned();
    action.volumes[0].container_path = "/data/typesense".to_owned();

    let result = DockerServiceSpec::from_proto(action);

    assert!(result.is_ok());
}

#[test]
fn rejects_host_path_parent_or_current_components() {
    for host_path in [
        "/var/lib/reallyme/nats/../other-service/data",
        "/var/lib/reallyme/nats/./data",
    ] {
        let mut action = sample_action();
        action.volumes[0].host_path = host_path.to_owned();

        let result = DockerServiceSpec::from_proto(action);

        assert!(result.is_err());
    }
}

#[test]
fn rejects_image_ref_with_unsupported_equals_character() {
    let mut action = sample_action();
    action.image_ref = "ams.vultrcr.com/reallyme/nats=2.14.0-amd64".to_owned();

    let result = DockerServiceSpec::from_proto(action);

    assert!(result.is_err());
}

#[test]
fn rejects_localhost_http_health_probe_target() {
    let mut action = sample_action();
    action.health_probes[0].target = "http://localhost:8222/healthz".to_owned();

    let result = DockerServiceSpec::from_proto(action);

    assert!(result.is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_symlinked_volume_host_path_escape() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error:?}"));
    let allowed = dir.path().join("allowed");
    let outside = dir.path().join("outside");
    let link = allowed.join("escape");
    tokio::fs::create_dir_all(allowed.as_path())
        .await
        .unwrap_or_else(|error| panic!("create allowed: {error:?}"));
    tokio::fs::create_dir_all(outside.as_path())
        .await
        .unwrap_or_else(|error| panic!("create outside: {error:?}"));
    std::os::unix::fs::symlink(outside.as_path(), link.as_path())
        .unwrap_or_else(|error| panic!("symlink: {error:?}"));
    let canonical_allowed = tokio::fs::canonicalize(allowed.as_path())
        .await
        .unwrap_or_else(|error| panic!("canonical allowed: {error:?}"));

    let result = validate_host_path_on_disk(link.as_path(), &[canonical_allowed]).await;

    assert!(result.is_err());
}

#[test]
fn rejects_config_file_path_traversal() {
    let mut action = sample_action();
    action.config_files[0].relative_path = "../nats.conf".to_owned();

    let result = DockerServiceSpec::from_proto(action);

    assert!(result.is_err());
}

#[test]
fn rejects_secret_file_path_traversal() {
    let mut action = sample_action();
    action.secret_files[0].relative_path = "/etc/passwd".to_owned();

    let result = DockerServiceSpec::from_proto(action);

    assert!(result.is_err());
}

#[test]
fn resolved_secret_file_debug_redacts_contents() {
    let resolved = ResolvedSecretFile {
        relative_path: PathBuf::from("auth/token"),
        contents: SecretString::new("service-secret-token".into()),
    };

    assert!(!format!("{resolved:?}").contains("service-secret-token"));
}

#[tokio::test]
async fn writes_secret_files_with_restrictive_permissions() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error:?}"));
    let path = dir.path().join("secret-token");

    write_secret_file_atomic(path.as_path(), b"secret")
        .await
        .unwrap_or_else(|error| {
            panic!("secret write: {error:?}");
        });

    let contents = tokio::fs::read_to_string(path.as_path())
        .await
        .unwrap_or_else(|error| panic!("read secret: {error:?}"));
    assert_eq!(contents, "secret");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = tokio::fs::metadata(path.as_path())
            .await
            .unwrap_or_else(|error| panic!("metadata: {error:?}"));
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }
}

#[tokio::test]
async fn health_gate_retries_until_service_becomes_ready() {
    let reservation = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|error| panic!("reserve port: {error:?}"));
    let address = reservation
        .local_addr()
        .unwrap_or_else(|error| panic!("reserved address: {error:?}"));
    drop(reservation);

    let server = tokio::spawn(async move {
        sleep(Duration::from_millis(150)).await;
        let listener = TcpListener::bind(address)
            .await
            .unwrap_or_else(|error| panic!("bind delayed listener: {error:?}"));
        let _connection = listener
            .accept()
            .await
            .unwrap_or_else(|error| panic!("accept probe: {error:?}"));
    });

    let mut spec = DockerServiceSpec::from_proto(sample_action())
        .unwrap_or_else(|error| panic!("valid docker service: {error:?}"));
    spec.health_probes = vec![HealthProbe {
        name: "delayed-tcp".to_owned(),
        kind: HealthProbeKind::Tcp,
        target: address.to_string(),
        expected_http_status: 200,
    }];

    let result = health_gate(&spec, Duration::from_secs(2)).await;
    server
        .await
        .unwrap_or_else(|error| panic!("probe server join: {error:?}"));

    assert!(result.is_ok());
}

#[tokio::test]
async fn copies_rollback_files_preserving_permissions() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error:?}"));
    let source = dir.path().join("source");
    let destination = dir.path().join("destination");
    tokio::fs::create_dir_all(source.as_path())
        .await
        .unwrap_or_else(|error| panic!("create source: {error:?}"));
    let secret = source.join("secret.env");
    write_secret_file_atomic(secret.as_path(), b"TOKEN=value")
        .await
        .unwrap_or_else(|error| panic!("secret write: {error:?}"));

    copy_dir_recursive(source.as_path(), destination.as_path())
        .await
        .unwrap_or_else(|error| panic!("copy rollback: {error:?}"));

    let copied = tokio::fs::read_to_string(destination.join("secret.env").as_path())
        .await
        .unwrap_or_else(|error| panic!("read copied: {error:?}"));
    assert_eq!(copied, "TOKEN=value");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = tokio::fs::metadata(destination.join("secret.env").as_path())
            .await
            .unwrap_or_else(|error| panic!("metadata: {error:?}"));
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }
}
