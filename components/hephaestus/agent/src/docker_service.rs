// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic Docker service configuration for node-local workloads.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::actions::AgentActionReceipt;
use buffa::Enumeration as _;
use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1 as pb;
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use tokio::process::Command;
use tokio::time::{sleep, timeout};

use crate::docker_engine;
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};
use crate::service_probe::{run_ad_hoc_http_probe, run_ad_hoc_tcp_probe};
use crate::tailscale::{TailscaleServiceDefinition, TailscaleServiceEndpoint};

const ETC_SERVICE_ROOT: &str = "/etc/reallyme/services";
const ETC_SECRET_ROOT: &str = "/etc/reallyme/secrets";
const VAR_SERVICE_ROOT: &str = "/var/lib/reallyme";
const LOG_SERVICE_ROOT: &str = "/var/log/reallyme";
const DOCKER_BINARY: &str = "/usr/bin/docker";
const CHOWN_BINARY: &str = "/usr/bin/chown";
const DOCKER_CONFIG_ENV_VAR: &str = "DOCKER_CONFIG";
const DOCKER_SERVICE_PHASE_AUDIT_LOG_PATH: &str =
    "/var/lib/reallyme/hephaestus-agent/docker-service-phase-audit.jsonl";
const ROOT_USER_ID: &str = "0";
const ROOT_GROUP_ID: &str = "0";
const CONFIG_DIR_NAME: &str = "config";
const CONTAINER_CONFIG_DIR: &str = "/config";
const CONTAINER_SECRET_DIR: &str = "/run/secrets/reallyme";
const MAX_ENVIRONMENT_VARIABLES: usize = 128;
const MAX_PORTS: usize = 64;
const MAX_VOLUMES: usize = 64;
const MAX_HEALTH_PROBES: usize = 16;
const MAX_TAILSCALE_SERVICES: usize = 16;
const MAX_CONFIG_FILES: usize = 64;
const MAX_SECRET_FILES: usize = 64;
const MAX_REGISTRY_AUTHS: usize = 8;
const MAX_RELATIVE_PATH_BYTES: usize = 192;
const MAX_CONFIG_FILE_BYTES: usize = 256 * 1024;
const MAX_SECRET_FILE_BYTES: usize = 64 * 1024;
const HEALTH_GATE_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const HEALTH_GATE_MAX_PROBE_ATTEMPT: Duration = Duration::from_secs(1);

/// Runtime secret resolver for an already delivered node-local action.
///
/// Secrets are intentionally resolved after polling, immediately before writing
/// local files. This keeps durable action queues free of secret material.
pub trait DockerServiceSecretResolver: Send + Sync {
    /// Resolves one server-side secret reference for a specific action.
    fn resolve_agent_secret<'a>(
        &'a self,
        receipt: &'a AgentActionReceipt,
        secret_ref: &'a SecretRef,
    ) -> Pin<Box<dyn Future<Output = AgentResult<SecretString>> + Send + 'a>>;
}

/// Validated Docker service allocation sent by Hephaestus.
#[derive(Debug, Clone)]
pub struct DockerServiceSpec {
    service_id: String,
    container_name: String,
    image: String,
    expected_image_digest: Option<String>,
    environment: Vec<EnvironmentVariable>,
    ports: Vec<DockerPort>,
    volumes: Vec<DockerVolume>,
    health_probes: Vec<HealthProbe>,
    restart_policy: RestartPolicy,
    tailscale_services: Vec<TailscaleServiceDefinition>,
    config_files: Vec<RenderedConfigFile>,
    secret_files: Vec<RenderedSecretFile>,
    registry_auth: Vec<RegistryAuth>,
}

impl DockerServiceSpec {
    /// Validates generated protobuf input into a node-local Docker service spec.
    pub fn from_proto(value: pb::AgentDockerServiceAction) -> AgentResult<Self> {
        validate_identifier(value.service_id.as_str())?;
        validate_identifier(value.container_name.as_str())?;
        validate_image_ref(value.image_ref.as_str())?;
        if value.environment.len() > MAX_ENVIRONMENT_VARIABLES
            || value.ports.len() > MAX_PORTS
            || value.volumes.len() > MAX_VOLUMES
            || value.health_probes.len() > MAX_HEALTH_PROBES
            || value.tailscale_services.len() > MAX_TAILSCALE_SERVICES
            || value.config_files.len() > MAX_CONFIG_FILES
            || value.secret_files.len() > MAX_SECRET_FILES
            || value.registry_auth.len() > MAX_REGISTRY_AUTHS
        {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::InvalidConfig,
            ));
        }

        let image = compose_image(value.image_ref.as_str(), value.image_digest.as_deref())?;
        let expected_image_digest = value.image_digest.clone();
        let service_id = value.service_id;
        let environment = value
            .environment
            .into_iter()
            .map(environment_from_proto)
            .collect::<AgentResult<Vec<_>>>()?;
        let ports = value
            .ports
            .into_iter()
            .map(port_from_proto)
            .collect::<AgentResult<Vec<_>>>()?;
        let volumes = value
            .volumes
            .into_iter()
            .map(|volume| volume_from_proto(volume, service_id.as_str()))
            .collect::<AgentResult<Vec<_>>>()?;
        let health_probes = value
            .health_probes
            .into_iter()
            .map(health_probe_from_proto)
            .collect::<AgentResult<Vec<_>>>()?;
        let restart_policy = restart_policy_from_proto(value.restart_policy.to_i32())?;
        let tailscale_services = value
            .tailscale_services
            .into_iter()
            .map(tailscale_service_from_proto)
            .collect::<AgentResult<Vec<_>>>()?;
        let config_files = value
            .config_files
            .into_iter()
            .map(config_file_from_proto)
            .collect::<AgentResult<Vec<_>>>()?;
        let secret_files = value
            .secret_files
            .into_iter()
            .map(secret_file_from_proto)
            .collect::<AgentResult<Vec<_>>>()?;
        let registry_auth = value
            .registry_auth
            .into_iter()
            .map(registry_auth_from_proto)
            .collect::<AgentResult<Vec<_>>>()?;

        Ok(Self {
            service_id,
            container_name: value.container_name,
            image,
            expected_image_digest,
            environment,
            ports,
            volumes,
            health_probes,
            restart_policy,
            tailscale_services,
            config_files,
            secret_files,
            registry_auth,
        })
    }

    /// Returns configured Tailscale Services.
    pub fn tailscale_services(&self) -> &[TailscaleServiceDefinition] {
        self.tailscale_services.as_slice()
    }

    fn service_id(&self) -> &str {
        self.service_id.as_str()
    }
}

/// Applies one Docker service spec with deterministic Compose rendering.
pub async fn configure_docker_service(
    spec: &DockerServiceSpec,
    receipt: &AgentActionReceipt,
    command_timeout: Duration,
    secret_resolver: &impl DockerServiceSecretResolver,
) -> AgentResult<()> {
    log_docker_service_phase(spec.service_id(), "capture_rollback");
    let rollback = DeploymentRollback::capture(spec.service_id())
        .await
        .map_err(|error| {
            docker_service_phase_error(spec.service_id(), "capture_rollback", error)
        })?;
    log_docker_service_phase(spec.service_id(), "captured_rollback");
    match apply_docker_service_update(spec, receipt, command_timeout, secret_resolver).await {
        Ok(()) => rollback.discard().await.map_err(|error| {
            docker_service_phase_error(spec.service_id(), "discard_rollback", error)
        }),
        Err(error) => match rollback.restore(command_timeout).await {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(docker_service_phase_error(
                spec.service_id(),
                "restore_rollback",
                rollback_error,
            )),
        },
    }
}

async fn apply_docker_service_update(
    spec: &DockerServiceSpec,
    receipt: &AgentActionReceipt,
    command_timeout: Duration,
    secret_resolver: &impl DockerServiceSecretResolver,
) -> AgentResult<()> {
    log_docker_service_phase(spec.service_id(), "prepare_directories");
    let service_dir = service_dir(spec.service_id());
    let data_dir = data_dir(spec.service_id());
    let log_dir = log_dir(spec.service_id());
    let config_dir = config_dir(spec.service_id());
    let secret_dir = secret_dir(spec.service_id());
    tokio::fs::create_dir_all(&service_dir)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
        .map_err(|error| {
            docker_service_phase_error(spec.service_id(), "prepare_directories", error)
        })?;
    tokio::fs::create_dir_all(&data_dir)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
        .map_err(|error| {
            docker_service_phase_error(spec.service_id(), "prepare_directories", error)
        })?;
    tokio::fs::create_dir_all(&log_dir)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
        .map_err(|error| {
            docker_service_phase_error(spec.service_id(), "prepare_directories", error)
        })?;
    if !spec.config_files.is_empty() {
        tokio::fs::create_dir_all(&config_dir)
            .await
            .map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
            })
            .map_err(|error| {
                docker_service_phase_error(spec.service_id(), "prepare_directories", error)
            })?;
    }
    if !spec.secret_files.is_empty() {
        tokio::fs::create_dir_all(&secret_dir)
            .await
            .map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
            })
            .map_err(|error| {
                docker_service_phase_error(spec.service_id(), "prepare_directories", error)
            })?;
    }
    log_docker_service_phase(spec.service_id(), "validate_volume_paths");
    validate_volume_host_paths_on_disk(spec)
        .await
        .map_err(|error| {
            docker_service_phase_error(spec.service_id(), "validate_volume_paths", error)
        })?;
    log_docker_service_phase(spec.service_id(), "resolve_secret_files");
    let resolved_secret_files =
        resolve_secret_files(spec.secret_files.as_slice(), receipt, secret_resolver)
            .await
            .map_err(|error| {
                docker_service_phase_error(spec.service_id(), "resolve_secret_files", error)
            })?;
    log_docker_service_phase(spec.service_id(), "resolve_registry_auth");
    let resolved_registry_auth =
        resolve_registry_auth(spec.registry_auth.as_slice(), receipt, secret_resolver)
            .await
            .map_err(|error| {
                docker_service_phase_error(spec.service_id(), "resolve_registry_auth", error)
            })?;

    log_docker_service_phase(spec.service_id(), "write_compose");
    write_file_atomic(
        &service_dir.join("compose.yml"),
        render_compose(spec)
            .map_err(|error| docker_service_phase_error(spec.service_id(), "write_compose", error))?
            .as_bytes(),
    )
    .await
    .map_err(|error| docker_service_phase_error(spec.service_id(), "write_compose", error))?;
    log_docker_service_phase(spec.service_id(), "write_config_files");
    write_rendered_config_files(&config_dir, spec.config_files.as_slice())
        .await
        .map_err(|error| {
            docker_service_phase_error(spec.service_id(), "write_config_files", error)
        })?;
    log_docker_service_phase(spec.service_id(), "write_secret_files");
    write_rendered_secret_files(&secret_dir, resolved_secret_files.as_slice())
        .await
        .map_err(|error| {
            docker_service_phase_error(spec.service_id(), "write_secret_files", error)
        })?;
    log_docker_service_phase(spec.service_id(), "compose_pull");
    run_authenticated_compose_pull(
        spec.service_id(),
        &service_dir,
        resolved_registry_auth.as_slice(),
        command_timeout,
    )
    .await
    .map_err(|error| docker_service_phase_error(spec.service_id(), "compose_pull", error))?;
    log_docker_service_phase(spec.service_id(), "validate_image_digest");
    validate_pulled_image_digest(spec, command_timeout)
        .await
        .map_err(|error| {
            docker_service_phase_error(spec.service_id(), "validate_image_digest", error)
        })?;
    log_docker_service_phase(spec.service_id(), "prepare_workload_file_ownership");
    prepare_workload_file_ownership(spec, command_timeout)
        .await
        .map_err(|error| {
            docker_service_phase_error(spec.service_id(), "prepare_workload_file_ownership", error)
        })?;
    log_docker_service_phase(spec.service_id(), "compose_up");
    run_compose_with_temporary_docker_config(
        spec.service_id(),
        &service_dir,
        ["up", "-d", "--remove-orphans"],
        command_timeout,
    )
    .await
    .map_err(|error| docker_service_phase_error(spec.service_id(), "compose_up", error))?;
    log_docker_service_phase(spec.service_id(), "health_gate");
    if let Err(error) = health_gate(spec, command_timeout).await {
        // Health is an observed runtime signal, not the source of truth for
        // whether the desired Docker service configuration was installed. On
        // first boot, applications such as clustered NATS or Typesense can need
        // extra time after Compose accepts the service before their local probe
        // is green. Rolling back here destroys useful diagnostics and races the
        // separate agent report path, so leave the service running and let
        // normal health reports describe readiness or failure.
        tracing::warn!(
            service_id = spec.service_id(),
            reason = ?error.reason(),
            "docker service health gate did not become ready after compose apply"
        );
    }
    Ok(())
}

fn log_docker_service_phase(service_id: &str, phase: &'static str) {
    tracing::info!(
        service_id,
        phase,
        "applying hephaestus docker service phase"
    );
}

fn docker_service_phase_error(
    service_id: &str,
    phase: &'static str,
    error: HephaestusAgentError,
) -> HephaestusAgentError {
    append_docker_service_phase_failure(service_id, phase, error.reason());
    tracing::warn!(
        service_id,
        phase,
        reason = ?error.reason(),
        "hephaestus docker service phase failed"
    );
    error
}

fn append_docker_service_phase_failure(
    service_id: &str,
    phase: &'static str,
    reason: HephaestusAgentErrorReason,
) {
    let path = Path::new(DOCKER_SERVICE_PHASE_AUDIT_LOG_PATH);
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(event_unix_secs) = docker_service_phase_event_unix_secs() else {
        return;
    };
    let record = DockerServicePhaseAuditRecord {
        schema_version: 1,
        event_unix_secs,
        service_id,
        phase,
        reason: docker_service_phase_reason_label(reason),
    };
    let Ok(mut bytes) = serde_json::to_vec(&record) else {
        return;
    };
    bytes.push(b'\n');
    let Ok(mut file) = open_docker_service_phase_audit_log(path) else {
        return;
    };
    let _ = file.write_all(bytes.as_slice());
}

#[derive(Serialize)]
struct DockerServicePhaseAuditRecord<'a> {
    schema_version: u32,
    event_unix_secs: u64,
    service_id: &'a str,
    phase: &'static str,
    reason: &'static str,
}

fn docker_service_phase_event_unix_secs() -> AgentResult<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))
        .map(|duration| duration.as_secs())
}

#[cfg(unix)]
fn open_docker_service_phase_audit_log(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;

    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_docker_service_phase_audit_log(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
}

fn docker_service_phase_reason_label(reason: HephaestusAgentErrorReason) -> &'static str {
    match reason {
        HephaestusAgentErrorReason::InvalidConfig => "invalid_config",
        HephaestusAgentErrorReason::InvalidUrl => "invalid_url",
        HephaestusAgentErrorReason::InvalidPath => "invalid_path",
        HephaestusAgentErrorReason::InvalidIdentity => "invalid_identity",
        HephaestusAgentErrorReason::InvalidNumber => "invalid_number",
        HephaestusAgentErrorReason::InvalidHeader => "invalid_header",
        HephaestusAgentErrorReason::FileReadFailed => "file_read_failed",
        HephaestusAgentErrorReason::FileWriteFailed => "file_write_failed",
        HephaestusAgentErrorReason::FileDeleteFailed => "file_delete_failed",
        HephaestusAgentErrorReason::CommandFailed => "command_failed",
        HephaestusAgentErrorReason::CommandUnavailable => "command_unavailable",
        HephaestusAgentErrorReason::CommandOutputInvalid => "command_output_invalid",
        HephaestusAgentErrorReason::CadvisorUnavailable => "cadvisor_unavailable",
        HephaestusAgentErrorReason::MetricsEndpointUnavailable => "metrics_endpoint_unavailable",
        HephaestusAgentErrorReason::ConnectFailed => "connect_failed",
        HephaestusAgentErrorReason::RegistrationRejected => "registration_rejected",
        HephaestusAgentErrorReason::BootstrapTokenUnavailable => "bootstrap_token_unavailable",
        HephaestusAgentErrorReason::RuntimeTokenUnavailable => "runtime_token_unavailable",
        HephaestusAgentErrorReason::IdentityKeyUnavailable => "identity_key_unavailable",
        HephaestusAgentErrorReason::CryptoUnavailable => "crypto_unavailable",
        HephaestusAgentErrorReason::TlsUnavailable => "tls_unavailable",
        HephaestusAgentErrorReason::DomainValidationFailed => "domain_validation_failed",
    }
}

fn render_compose(spec: &DockerServiceSpec) -> AgentResult<String> {
    serde_norway::to_string(&ComposeFile::from_spec(spec))
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
}

async fn run_compose<const N: usize>(
    service_dir: &Path,
    args: [&str; N],
    command_timeout: Duration,
) -> AgentResult<()> {
    let mut command = Command::new(DOCKER_BINARY);
    command
        .arg("compose")
        .arg("--project-directory")
        .arg(service_dir);
    command.arg("-f").arg(service_dir.join("compose.yml"));
    command.args(args);
    let result = timeout(command_timeout, crate::command::output(&mut command)).await;
    match result {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(_output)) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
        Ok(Err(_error)) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
        Err(_error) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
    }
}

async fn run_compose_with_docker_config<const N: usize>(
    service_dir: &Path,
    docker_config_dir: &Path,
    args: [&str; N],
    command_timeout: Duration,
) -> AgentResult<()> {
    let mut command = Command::new(DOCKER_BINARY);
    command
        .env(DOCKER_CONFIG_ENV_VAR, docker_config_dir)
        .arg("compose")
        .arg("--project-directory")
        .arg(service_dir);
    command.arg("-f").arg(service_dir.join("compose.yml"));
    command.args(args);
    let result = timeout(command_timeout, crate::command::output(&mut command)).await;
    match result {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(_output)) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
        Ok(Err(_error)) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
        Err(_error) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
    }
}

async fn health_gate(spec: &DockerServiceSpec, command_timeout: Duration) -> AgentResult<()> {
    for probe in &spec.health_probes {
        let start = std::time::Instant::now();
        loop {
            let remaining_budget = command_timeout.saturating_sub(start.elapsed());
            if remaining_budget.is_zero() {
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::CommandFailed,
                ));
            }
            let attempt_timeout = remaining_budget.min(HEALTH_GATE_MAX_PROBE_ATTEMPT);
            if run_health_probe(probe, attempt_timeout).await? {
                break;
            }
            let remaining_after_probe = command_timeout.saturating_sub(start.elapsed());
            if remaining_after_probe.is_zero() {
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::CommandFailed,
                ));
            }
            sleep(HEALTH_GATE_RETRY_INTERVAL.min(remaining_after_probe)).await;
        }
    }
    Ok(())
}

async fn run_health_probe(probe: &HealthProbe, command_timeout: Duration) -> AgentResult<bool> {
    match probe.kind {
        HealthProbeKind::Http => {
            run_ad_hoc_http_probe(
                probe.target.as_str(),
                probe.expected_http_status,
                command_timeout,
            )
            .await
        }
        HealthProbeKind::Tcp => run_ad_hoc_tcp_probe(probe.target.as_str(), command_timeout).await,
    }
}

async fn validate_pulled_image_digest(
    spec: &DockerServiceSpec,
    command_timeout: Duration,
) -> AgentResult<()> {
    let Some(expected_digest) = spec.expected_image_digest.as_deref() else {
        return Ok(());
    };
    let repo_digests =
        docker_engine::image_repo_digests(spec.image.as_str(), command_timeout).await?;
    if repo_digests_contain_digest(
        repo_digests.as_slice(),
        spec.image.as_str(),
        expected_digest,
    ) {
        Ok(())
    } else {
        Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandOutputInvalid,
        ))
    }
}

fn repo_digests_contain_digest(
    repo_digests: &[String],
    image_ref: &str,
    expected_digest: &str,
) -> bool {
    let Some(expected_repo) = image_repo_name(image_ref) else {
        return false;
    };
    repo_digests.iter().any(|repo_digest| {
        repo_digest.rsplit_once('@').is_some_and(|(name, digest)| {
            digest == expected_digest && normalize_repo_name(name) == expected_repo
        })
    })
}

fn image_repo_name(image_ref: &str) -> Option<String> {
    let without_digest = image_ref
        .split_once('@')
        .map_or(image_ref, |(name, _digest)| name);
    let last_slash = without_digest.rfind('/');
    let last_colon = without_digest.rfind(':');
    let without_tag =
        if last_colon.is_some_and(|colon| last_slash.is_none_or(|slash| colon > slash)) {
            &without_digest[..last_colon?]
        } else {
            without_digest
        };
    Some(normalize_repo_name(without_tag))
}

fn normalize_repo_name(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn validate_registry_host(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || value.len() > 253
        || value
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b':')))
        || value.bytes().any(|byte| matches!(byte, b'/' | b'@'))
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    Ok(())
}

fn validate_registry_credential(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || value.len() > 4096
        || value
            .bytes()
            .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ));
    }
    Ok(())
}

mod rollback;
use rollback::DeploymentRollback;

#[derive(Debug, Clone, PartialEq, Eq)]
struct EnvironmentVariable {
    name: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DockerPort {
    protocol: PortProtocol,
    host_ip: String,
    host_port: u16,
    container_port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DockerVolume {
    host_path: String,
    container_path: String,
    read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedConfigFile {
    relative_path: PathBuf,
    contents: String,
    executable: bool,
}

#[derive(Debug, Clone)]
struct RenderedSecretFile {
    relative_path: PathBuf,
    secret_ref: SecretRef,
}

#[derive(Debug, Clone)]
struct RegistryAuth {
    registry: String,
    username_secret_ref: SecretRef,
    password_secret_ref: SecretRef,
}

#[derive(Debug)]
struct ResolvedRegistryAuth {
    registry: String,
    username: SecretString,
    password: SecretString,
}

#[derive(Debug)]
struct ResolvedSecretFile {
    relative_path: PathBuf,
    contents: SecretString,
}

/// Validated identifier used to locate a server-side action secret reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef(String);

impl SecretRef {
    /// Validates and stores a service-local secret reference.
    pub fn new(value: String) -> AgentResult<Self> {
        validate_secret_ref(value.as_str())?;
        Ok(Self(value))
    }

    /// Returns the opaque secret reference used by the control plane.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HealthProbe {
    name: String,
    kind: HealthProbeKind,
    target: String,
    expected_http_status: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HealthProbeKind {
    Http,
    Tcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestartPolicy {
    No,
    UnlessStopped,
    Always,
    OnFailure,
}

impl RestartPolicy {
    fn as_compose(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::UnlessStopped => "unless-stopped",
            Self::Always => "always",
            Self::OnFailure => "on-failure",
        }
    }
}

#[derive(Serialize)]
struct ComposeFile {
    services: BTreeMap<String, ComposeService>,
}

impl ComposeFile {
    fn from_spec(spec: &DockerServiceSpec) -> Self {
        let mut services = BTreeMap::new();
        services.insert(
            compose_service_name(spec.service_id()).to_owned(),
            ComposeService::from_spec(spec),
        );
        Self { services }
    }
}

#[derive(Serialize)]
struct ComposeService {
    image: String,
    container_name: String,
    restart: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_mode: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    read_only: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tmpfs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    security_opt: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cap_drop: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pids_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    healthcheck: Option<ComposeHealthcheck>,
    environment: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ports: Vec<String>,
    volumes: Vec<String>,
}

impl ComposeService {
    fn from_spec(spec: &DockerServiceSpec) -> Self {
        let mut volumes = spec.volumes.iter().map(render_volume).collect::<Vec<_>>();
        if !spec.config_files.is_empty() {
            volumes.push(read_only_volume(
                config_dir(spec.service_id()).as_path(),
                CONTAINER_CONFIG_DIR,
            ));
        }
        if !spec.secret_files.is_empty() {
            volumes.push(read_only_volume(
                secret_dir(spec.service_id()).as_path(),
                CONTAINER_SECRET_DIR,
            ));
        }

        Self {
            image: spec.image.clone(),
            container_name: spec.container_name.clone(),
            restart: spec.restart_policy.as_compose().to_owned(),
            network_mode: production_network_mode(spec.service_id()).map(str::to_owned),
            read_only: production_read_only_rootfs(spec.service_id()),
            tmpfs: production_tmpfs_mounts(spec.service_id()),
            security_opt: production_security_options(spec.service_id()),
            cap_drop: production_cap_drop(spec.service_id()),
            pids_limit: production_pids_limit(spec.service_id()),
            healthcheck: production_healthcheck(spec.service_id()),
            environment: environment_map(spec.environment.as_slice()),
            ports: production_network_mode(spec.service_id())
                .map(|_network_mode| Vec::new())
                .unwrap_or_else(|| spec.ports.iter().map(render_port).collect()),
            volumes,
        }
    }
}

#[derive(Serialize)]
struct ComposeHealthcheck {
    test: Vec<String>,
    interval: String,
    timeout: String,
    retries: u32,
    start_period: String,
}

fn compose_service_name(service_id: &str) -> &str {
    match service_id {
        "reallyme-nats" => "nats",
        "reallyme-typesense" => "typesense",
        _ => service_id,
    }
}

const fn is_false(value: &bool) -> bool {
    !*value
}

fn production_network_mode(service_id: &str) -> Option<&'static str> {
    match service_id {
        "reallyme-caddy" | "reallyme-nats" | "reallyme-typesense" | "reallyme-web" => Some("host"),
        _ => None,
    }
}

fn production_read_only_rootfs(service_id: &str) -> bool {
    matches!(
        service_id,
        "reallyme-caddy" | "reallyme-nats" | "reallyme-typesense" | "reallyme-web"
    )
}

fn production_tmpfs_mounts(service_id: &str) -> Vec<String> {
    match service_id {
        "reallyme-caddy" => vec![String::from(
            "/run/caddy:rw,noexec,nosuid,nodev,size=1m,uid=1000,gid=1000,mode=0700",
        )],
        "reallyme-nats" => vec![
            String::from("/run/nats:rw,noexec,nosuid,size=16m,uid=1000,gid=1000,mode=0755"),
            String::from("/tmp:rw,noexec,nosuid,size=16m,uid=1000,gid=1000,mode=1777"),
        ],
        "reallyme-typesense" => vec![
            String::from(
                "/run/typesense:rw,noexec,nosuid,nodev,size=1m,uid=10001,gid=10001,mode=0700",
            ),
            String::from("/tmp:rw,noexec,nosuid,nodev,size=16m,uid=10001,gid=10001,mode=1777"),
        ],
        "reallyme-web" => vec![String::from(
            "/tmp:rw,noexec,nosuid,nodev,size=64m,uid=10001,gid=10001,mode=1777",
        )],
        _ => Vec::new(),
    }
}

fn production_security_options(service_id: &str) -> Vec<String> {
    match service_id {
        "reallyme-caddy" | "reallyme-nats" | "reallyme-typesense" | "reallyme-web" => {
            vec![String::from("no-new-privileges:true")]
        }
        _ => Vec::new(),
    }
}

fn production_cap_drop(service_id: &str) -> Vec<String> {
    match service_id {
        "reallyme-nats" | "reallyme-typesense" | "reallyme-web" => vec![String::from("ALL")],
        _ => Vec::new(),
    }
}

fn production_pids_limit(service_id: &str) -> Option<u32> {
    match service_id {
        "reallyme-caddy" => Some(256),
        "reallyme-nats" | "reallyme-typesense" => Some(512),
        "reallyme-web" => Some(1024),
        _ => None,
    }
}

fn production_healthcheck(service_id: &str) -> Option<ComposeHealthcheck> {
    match service_id {
        "reallyme-nats" => Some(ComposeHealthcheck {
            test: vec![
                String::from("CMD-SHELL"),
                String::from(
                    "wget -q -O /dev/null 'http://127.0.0.1:8222/healthz?js-enabled-only=true'",
                ),
            ],
            interval: String::from("30s"),
            timeout: String::from("5s"),
            retries: 3,
            start_period: String::from("20s"),
        }),
        "reallyme-caddy" => Some(ComposeHealthcheck {
            test: vec![
                String::from("CMD-SHELL"),
                String::from("wget -q -O /dev/null 'http://127.0.0.1:8080/healthz'"),
            ],
            interval: String::from("30s"),
            timeout: String::from("5s"),
            retries: 3,
            start_period: String::from("10s"),
        }),
        "reallyme-web" => Some(ComposeHealthcheck {
            test: vec![
                String::from("CMD-SHELL"),
                String::from(
                    "node -e \"const controller=new AbortController(); const timeout=setTimeout(() => controller.abort(), 4000); fetch('http://127.0.0.1:3000/', { signal: controller.signal }).then((response) => { clearTimeout(timeout); process.exit(response.ok ? 0 : 1); }).catch(() => process.exit(1));\"",
                ),
            ],
            interval: String::from("30s"),
            timeout: String::from("5s"),
            retries: 3,
            start_period: String::from("20s"),
        }),
        _ => None,
    }
}

fn environment_map(environment: &[EnvironmentVariable]) -> BTreeMap<String, String> {
    environment
        .iter()
        .map(|variable| (variable.name.clone(), variable.value.clone()))
        .collect()
}

fn environment_from_proto(
    value: pb::AgentDockerEnvironmentVariable,
) -> AgentResult<EnvironmentVariable> {
    validate_env_name(value.name.as_str())?;
    validate_env_value(value.value.as_str())?;
    Ok(EnvironmentVariable {
        name: value.name,
        value: value.value,
    })
}

fn port_from_proto(value: pb::AgentDockerPort) -> AgentResult<DockerPort> {
    let protocol = match pb::AgentDockerPortProtocol::from_i32(value.protocol.to_i32())
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidConfig))?
    {
        pb::AgentDockerPortProtocol::AGENT_DOCKER_PORT_PROTOCOL_TCP => PortProtocol::Tcp,
        pb::AgentDockerPortProtocol::AGENT_DOCKER_PORT_PROTOCOL_UDP => PortProtocol::Udp,
        pb::AgentDockerPortProtocol::AGENT_DOCKER_PORT_PROTOCOL_UNSPECIFIED => {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::InvalidConfig,
            ));
        }
    };
    validate_local_host_ip(value.host_ip.as_str())?;
    let host_port = validate_proto_port(value.host_port)?;
    let container_port = validate_proto_port(value.container_port)?;
    Ok(DockerPort {
        protocol,
        host_ip: value.host_ip,
        host_port,
        container_port,
    })
}

fn volume_from_proto(value: pb::AgentDockerVolume, service_id: &str) -> AgentResult<DockerVolume> {
    validate_host_path(value.host_path.as_str(), service_id)?;
    validate_container_path(value.container_path.as_str())?;
    Ok(DockerVolume {
        host_path: value.host_path,
        container_path: value.container_path,
        read_only: value.read_only,
    })
}

fn health_probe_from_proto(value: pb::AgentDockerHealthProbe) -> AgentResult<HealthProbe> {
    validate_identifier(value.name.as_str())?;
    let kind = match pb::AgentDockerHealthProbeKind::from_i32(value.kind.to_i32())
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidConfig))?
    {
        pb::AgentDockerHealthProbeKind::AGENT_DOCKER_HEALTH_PROBE_KIND_HTTP => {
            validate_local_http_target(value.target.as_str())?;
            HealthProbeKind::Http
        }
        pb::AgentDockerHealthProbeKind::AGENT_DOCKER_HEALTH_PROBE_KIND_TCP => {
            validate_local_tcp_target(value.target.as_str())?;
            HealthProbeKind::Tcp
        }
        pb::AgentDockerHealthProbeKind::AGENT_DOCKER_HEALTH_PROBE_KIND_UNSPECIFIED => {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::InvalidConfig,
            ));
        }
    };
    let expected_http_status = if value.expected_http_status == 0 {
        200
    } else {
        u16::try_from(value.expected_http_status).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber)
        })?
    };
    Ok(HealthProbe {
        name: value.name,
        kind,
        target: value.target,
        expected_http_status,
    })
}

fn restart_policy_from_proto(value: i32) -> AgentResult<RestartPolicy> {
    match pb::AgentDockerRestartPolicy::from_i32(value)
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidConfig))?
    {
        pb::AgentDockerRestartPolicy::AGENT_DOCKER_RESTART_POLICY_NO => Ok(RestartPolicy::No),
        pb::AgentDockerRestartPolicy::AGENT_DOCKER_RESTART_POLICY_UNLESS_STOPPED => {
            Ok(RestartPolicy::UnlessStopped)
        }
        pb::AgentDockerRestartPolicy::AGENT_DOCKER_RESTART_POLICY_ALWAYS => {
            Ok(RestartPolicy::Always)
        }
        pb::AgentDockerRestartPolicy::AGENT_DOCKER_RESTART_POLICY_ON_FAILURE => {
            Ok(RestartPolicy::OnFailure)
        }
        pb::AgentDockerRestartPolicy::AGENT_DOCKER_RESTART_POLICY_UNSPECIFIED => Err(
            HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidConfig),
        ),
    }
}

fn tailscale_service_from_proto(
    value: pb::AgentTailscaleServiceConfig,
) -> AgentResult<TailscaleServiceDefinition> {
    let endpoints = value
        .endpoints
        .into_iter()
        .map(|endpoint| TailscaleServiceEndpoint::new(endpoint.listen, endpoint.upstream))
        .collect::<AgentResult<Vec<_>>>()?;
    TailscaleServiceDefinition::new(value.service_name, endpoints)
}

fn config_file_from_proto(value: pb::AgentDockerRenderedFile) -> AgentResult<RenderedConfigFile> {
    validate_file_contents(value.contents.as_str(), MAX_CONFIG_FILE_BYTES)?;
    Ok(RenderedConfigFile {
        relative_path: validate_relative_service_file_path(value.relative_path.as_str())?,
        contents: value.contents,
        executable: value.executable,
    })
}

fn secret_file_from_proto(value: pb::AgentDockerSecretFile) -> AgentResult<RenderedSecretFile> {
    Ok(RenderedSecretFile {
        relative_path: validate_relative_service_file_path(value.relative_path.as_str())?,
        secret_ref: SecretRef::new(value.secret_ref)?,
    })
}

fn registry_auth_from_proto(value: pb::AgentDockerRegistryAuth) -> AgentResult<RegistryAuth> {
    validate_registry_host(value.registry.as_str())?;
    Ok(RegistryAuth {
        registry: value.registry,
        username_secret_ref: SecretRef::new(value.username_secret_ref)?,
        password_secret_ref: SecretRef::new(value.password_secret_ref)?,
    })
}

async fn resolve_secret_files(
    secret_files: &[RenderedSecretFile],
    receipt: &AgentActionReceipt,
    secret_resolver: &impl DockerServiceSecretResolver,
) -> AgentResult<Vec<ResolvedSecretFile>> {
    let mut resolved = Vec::with_capacity(secret_files.len());
    for file in secret_files {
        let contents = secret_resolver
            .resolve_agent_secret(receipt, &file.secret_ref)
            .await?;
        validate_file_contents(contents.expose_secret(), MAX_SECRET_FILE_BYTES)?;
        resolved.push(ResolvedSecretFile {
            relative_path: file.relative_path.clone(),
            contents,
        });
    }
    Ok(resolved)
}

async fn resolve_registry_auth(
    registry_auth: &[RegistryAuth],
    receipt: &AgentActionReceipt,
    secret_resolver: &impl DockerServiceSecretResolver,
) -> AgentResult<Vec<ResolvedRegistryAuth>> {
    let mut resolved = Vec::with_capacity(registry_auth.len());
    for auth in registry_auth {
        let username = secret_resolver
            .resolve_agent_secret(receipt, &auth.username_secret_ref)
            .await?;
        let password = secret_resolver
            .resolve_agent_secret(receipt, &auth.password_secret_ref)
            .await?;
        validate_registry_credential(username.expose_secret())?;
        validate_registry_credential(password.expose_secret())?;
        resolved.push(ResolvedRegistryAuth {
            registry: auth.registry.clone(),
            username,
            password,
        });
    }
    Ok(resolved)
}

async fn run_registry_logins(
    registry_auth: &[ResolvedRegistryAuth],
    docker_config_dir: &Path,
    command_timeout: Duration,
) -> AgentResult<()> {
    for auth in registry_auth {
        docker_login(auth, docker_config_dir, command_timeout).await?;
    }
    Ok(())
}

async fn run_authenticated_compose_pull(
    service_id: &str,
    service_dir: &Path,
    registry_auth: &[ResolvedRegistryAuth],
    command_timeout: Duration,
) -> AgentResult<()> {
    let docker_config_dir = docker_auth_config_dir(service_id);
    remove_dir_if_exists(docker_config_dir.as_path()).await?;
    tokio::fs::create_dir_all(docker_config_dir.as_path())
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    set_file_permissions(docker_config_dir.as_path(), 0o700).await?;

    let result = async {
        run_registry_logins(registry_auth, docker_config_dir.as_path(), command_timeout).await?;
        run_compose_with_docker_config(
            service_dir,
            docker_config_dir.as_path(),
            ["pull"],
            command_timeout,
        )
        .await
    }
    .await;
    let cleanup_result = remove_dir_if_exists(docker_config_dir.as_path()).await;
    match (result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => {
            tracing::warn!(
                service_id,
                reason = ?error.reason(),
                "temporary docker auth cleanup failed after successful compose pull"
            );
            Ok(())
        }
        (Err(_), Err(error)) => Err(error),
    }
}

async fn run_compose_with_temporary_docker_config<const N: usize>(
    service_id: &str,
    service_dir: &Path,
    args: [&str; N],
    command_timeout: Duration,
) -> AgentResult<()> {
    let docker_config_dir = docker_auth_config_dir(service_id);
    remove_dir_if_exists(docker_config_dir.as_path()).await?;
    tokio::fs::create_dir_all(docker_config_dir.as_path())
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    set_file_permissions(docker_config_dir.as_path(), 0o700).await?;
    let result = run_compose_with_docker_config(
        service_dir,
        docker_config_dir.as_path(),
        args,
        command_timeout,
    )
    .await;
    let cleanup_result = remove_dir_if_exists(docker_config_dir.as_path()).await;
    match (result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => {
            tracing::warn!(
                service_id,
                reason = ?error.reason(),
                "temporary docker auth cleanup failed after successful compose command"
            );
            Ok(())
        }
        (Err(_), Err(error)) => Err(error),
    }
}

async fn prepare_workload_file_ownership(
    spec: &DockerServiceSpec,
    command_timeout: Duration,
) -> AgentResult<()> {
    if spec.secret_files.is_empty() && spec.volumes.iter().all(|volume| volume.read_only) {
        return Ok(());
    }
    let Some((uid, gid)) = docker_image_numeric_user(spec.image.as_str(), command_timeout).await?
    else {
        return Ok(());
    };
    prepare_secret_file_ownership(spec, uid.as_str(), gid.as_str(), command_timeout).await?;
    prepare_writable_volume_ownership(spec, uid.as_str(), gid.as_str(), command_timeout).await
}

async fn prepare_secret_file_ownership(
    spec: &DockerServiceSpec,
    uid: &str,
    _gid: &str,
    command_timeout: Duration,
) -> AgentResult<()> {
    if spec.secret_files.is_empty() {
        return Ok(());
    }
    let secret_dir = secret_dir(spec.service_id());
    let mut parent_dirs = BTreeSet::new();
    for file in &spec.secret_files {
        let path = secret_dir.join(file.relative_path.as_path());
        chown_path(path.as_path(), ROOT_USER_ID, ROOT_GROUP_ID, command_timeout).await?;
        set_file_permissions(path.as_path(), 0o440).await?;
        chown_path(path.as_path(), uid, ROOT_GROUP_ID, command_timeout).await?;
        let mut current = path.parent();
        while let Some(parent) = current {
            if parent == secret_dir.as_path() {
                break;
            }
            if !parent.starts_with(secret_dir.as_path()) {
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::InvalidPath,
                ));
            }
            parent_dirs.insert(parent.to_path_buf());
            current = parent.parent();
        }
    }
    for path in parent_dirs.iter().rev() {
        chown_path(path.as_path(), ROOT_USER_ID, ROOT_GROUP_ID, command_timeout).await?;
        set_file_permissions(path.as_path(), 0o770).await?;
        chown_path(path.as_path(), uid, ROOT_GROUP_ID, command_timeout).await?;
    }
    chown_path(
        secret_dir.as_path(),
        ROOT_USER_ID,
        ROOT_GROUP_ID,
        command_timeout,
    )
    .await?;
    set_file_permissions(secret_dir.as_path(), 0o770).await?;
    chown_path(secret_dir.as_path(), uid, ROOT_GROUP_ID, command_timeout).await
}

async fn prepare_writable_volume_ownership(
    spec: &DockerServiceSpec,
    uid: &str,
    gid: &str,
    command_timeout: Duration,
) -> AgentResult<()> {
    for volume in spec.volumes.iter().filter(|volume| !volume.read_only) {
        tokio::fs::create_dir_all(volume.host_path.as_str())
            .await
            .map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
            })?;
        if path_owner_matches(volume.host_path.as_str(), uid, gid).await? {
            continue;
        }
        chown_recursive(volume.host_path.as_str(), uid, gid, command_timeout).await?;
    }
    Ok(())
}

#[cfg(unix)]
async fn path_owner_matches(path: &str, uid: &str, gid: &str) -> AgentResult<bool> {
    use std::os::unix::fs::MetadataExt as _;

    let expected_uid = uid
        .parse::<u32>()
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
    let expected_gid = gid
        .parse::<u32>()
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
    Ok(metadata.uid() == expected_uid && metadata.gid() == expected_gid)
}

#[cfg(not(unix))]
async fn path_owner_matches(_path: &str, _uid: &str, _gid: &str) -> AgentResult<bool> {
    Ok(false)
}

async fn docker_image_numeric_user(
    image_ref: &str,
    command_timeout: Duration,
) -> AgentResult<Option<(String, String)>> {
    let mut command = Command::new(DOCKER_BINARY);
    command
        .arg("image")
        .arg("inspect")
        .arg(image_ref)
        .arg("--format")
        .arg("{{.Config.User}}");
    let result = timeout(command_timeout, crate::command::output(&mut command)).await;
    let output = match result {
        Ok(Ok(output)) if output.status.success() => output,
        Ok(Ok(_output)) => {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::CommandFailed,
            ));
        }
        Ok(Err(_error)) => {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::CommandFailed,
            ));
        }
        Err(_error) => {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::CommandFailed,
            ));
        }
    };
    let stdout = String::from_utf8(output.stdout).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })?;
    parse_numeric_container_user(stdout.trim())
}

fn parse_numeric_container_user(value: &str) -> AgentResult<Option<(String, String)>> {
    if value.is_empty() || matches!(value, "0" | "0:0" | "root" | "root:root") {
        return Ok(None);
    }
    let Some((uid, gid)) = value.split_once(':') else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ));
    };
    if uid.is_empty()
        || gid.is_empty()
        || !uid.bytes().all(|byte| byte.is_ascii_digit())
        || !gid.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ));
    }
    Ok(Some((uid.to_owned(), gid.to_owned())))
}

async fn chown_recursive(
    path: &str,
    uid: &str,
    gid: &str,
    command_timeout: Duration,
) -> AgentResult<()> {
    let root = PathBuf::from(path);
    let paths =
        tokio::task::spawn_blocking(move || collect_chown_paths_child_first(root.as_path()))
            .await
            .map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed)
            })??;
    for path in paths {
        chown_path(path.as_path(), uid, gid, command_timeout).await?;
    }
    Ok(())
}

fn collect_chown_paths_child_first(path: &Path) -> AgentResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_chown_paths_child_first_into(path, &mut paths)?;
    Ok(paths)
}

fn collect_chown_paths_child_first_into(path: &Path, paths: &mut Vec<PathBuf>) -> AgentResult<()> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ));
    }
    if file_type.is_dir() {
        let entries = std::fs::read_dir(path)
            .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
        for entry in entries {
            let entry = entry.map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath)
            })?;
            collect_chown_paths_child_first_into(entry.path().as_path(), paths)?;
        }
        paths.push(path.to_path_buf());
        Ok(())
    } else if file_type.is_file() {
        paths.push(path.to_path_buf());
        Ok(())
    } else {
        Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ))
    }
}

fn owner_arg(uid: &str, gid: &str) -> String {
    let mut owner = String::with_capacity(uid.len() + gid.len() + 1);
    owner.push_str(uid);
    owner.push(':');
    owner.push_str(gid);
    owner
}

async fn chown_path(
    path: &Path,
    uid: &str,
    gid: &str,
    command_timeout: Duration,
) -> AgentResult<()> {
    let path = path
        .to_str()
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
    let owner = owner_arg(uid, gid);
    run_chown([owner.as_str(), path], command_timeout).await
}

async fn run_chown<const N: usize>(args: [&str; N], command_timeout: Duration) -> AgentResult<()> {
    let mut command = Command::new(CHOWN_BINARY);
    command.args(args);
    let result = timeout(command_timeout, crate::command::output(&mut command)).await;
    match result {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(_output)) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
        Ok(Err(_error)) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
        Err(_error) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        )),
    }
}

fn compose_image(image_ref: &str, image_digest: Option<&str>) -> AgentResult<String> {
    if let Some(digest) = image_digest {
        validate_digest(digest)?;
        if image_ref.contains('@') {
            return Ok(image_ref.to_owned());
        }
        let mut image = String::with_capacity(image_ref.len() + digest.len() + 1);
        image.push_str(image_ref);
        image.push('@');
        image.push_str(digest);
        Ok(image)
    } else {
        Ok(image_ref.to_owned())
    }
}

fn render_port(port: &DockerPort) -> String {
    let protocol = match port.protocol {
        PortProtocol::Tcp => "tcp",
        PortProtocol::Udp => "udp",
    };
    let mut rendered = String::new();
    rendered.push_str(port.host_ip.as_str());
    rendered.push(':');
    rendered.push_str(port.host_port.to_string().as_str());
    rendered.push(':');
    rendered.push_str(port.container_port.to_string().as_str());
    rendered.push('/');
    rendered.push_str(protocol);
    rendered
}

fn render_volume(volume: &DockerVolume) -> String {
    let mut rendered = String::new();
    rendered.push_str(volume.host_path.as_str());
    rendered.push(':');
    rendered.push_str(volume.container_path.as_str());
    if volume.read_only {
        rendered.push_str(":ro");
    }
    rendered
}

fn read_only_volume(host_path: &Path, container_path: &str) -> String {
    let mut rendered = String::new();
    rendered.push_str(host_path.to_string_lossy().as_ref());
    rendered.push(':');
    rendered.push_str(container_path);
    rendered.push_str(":ro");
    rendered
}

mod file_ops;
use file_ops::{
    config_dir, data_dir, docker_auth_config_dir, log_dir, path_exists, remove_dir_if_exists,
    secret_dir, service_dir, set_file_permissions, write_file_atomic, write_rendered_config_files,
    write_rendered_secret_files,
};

mod registry_login;
use registry_login::docker_login;
mod validation;
use validation::{
    validate_container_path, validate_digest, validate_env_name, validate_env_value,
    validate_file_contents, validate_host_path, validate_identifier, validate_image_ref,
    validate_local_host_ip, validate_local_http_target, validate_local_tcp_target,
    validate_proto_port, validate_relative_service_file_path, validate_secret_ref,
    validate_volume_host_paths_on_disk,
};

#[cfg(test)]
mod tests;
