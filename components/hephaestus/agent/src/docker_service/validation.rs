// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Component, Path, PathBuf};

use super::{
    DockerServiceSpec, MAX_RELATIVE_PATH_BYTES, data_dir, log_dir, path_exists, service_dir,
};
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

pub(super) fn validate_identifier(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || value.starts_with('-')
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    Ok(())
}

pub(super) fn validate_env_name(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        || value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_digit())
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    Ok(())
}

pub(super) fn validate_env_value(value: &str) -> AgentResult<()> {
    if value.len() > 8192
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

pub(super) fn validate_image_ref(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || value.len() > 255
        || value.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/' | b'@' | b'+'))
        })
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    Ok(())
}

pub(super) fn validate_digest(value: &str) -> AgentResult<()> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    Ok(())
}

pub(super) fn validate_proto_port(value: u32) -> AgentResult<u16> {
    let port = u16::try_from(value)
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
    if port == 0 {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidNumber,
        ));
    }
    // Low ports are intentionally allowed: workloads bind inside Docker and
    // the generated host mappings are loopback/private unless topology policy
    // explicitly selects public ingress.
    Ok(port)
}

pub(super) fn validate_local_host_ip(value: &str) -> AgentResult<()> {
    if !matches!(value, "127.0.0.1" | "0.0.0.0" | "::1") {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    Ok(())
}

pub(super) fn validate_host_path(value: &str, service_id: &str) -> AgentResult<()> {
    let path = Path::new(value);
    if value.as_bytes().contains(&0) || !path.is_absolute() || has_unsafe_path_segment(value) {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ));
    }
    for component in path.components() {
        match component {
            Component::RootDir | Component::Normal(_) => {}
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::InvalidPath,
                ));
            }
        }
    }
    let allowed = [
        service_dir(service_id),
        data_dir(service_id),
        log_dir(service_id),
    ];
    if allowed.iter().any(|prefix| path.starts_with(prefix)) {
        Ok(())
    } else {
        Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ))
    }
}

fn has_unsafe_path_segment(value: &str) -> bool {
    value
        .split('/')
        .any(|component| matches!(component, "." | ".."))
}

pub(super) async fn validate_volume_host_paths_on_disk(
    spec: &DockerServiceSpec,
) -> AgentResult<()> {
    let allowed = allowed_volume_mount_bases(spec.service_id()).await?;
    for volume in &spec.volumes {
        validate_host_path_on_disk(Path::new(volume.host_path.as_str()), allowed.as_slice())
            .await?;
    }
    Ok(())
}

async fn allowed_volume_mount_bases(service_id: &str) -> AgentResult<[PathBuf; 3]> {
    // Filesystem authority can change between deployments; never cache it.
    Ok([
        canonical_existing_path(service_dir(service_id).as_path()).await?,
        canonical_existing_path(data_dir(service_id).as_path()).await?,
        canonical_existing_path(log_dir(service_id).as_path()).await?,
    ])
}

pub(super) async fn validate_host_path_on_disk(
    path: &Path,
    allowed: &[PathBuf],
) -> AgentResult<()> {
    let canonical = canonical_volume_mount_basis(path).await?;
    if allowed
        .iter()
        .any(|allowed_prefix| canonical.starts_with(allowed_prefix))
    {
        Ok(())
    } else {
        Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ))
    }
}

async fn canonical_volume_mount_basis(path: &Path) -> AgentResult<PathBuf> {
    if path_exists(path).await {
        return canonical_existing_path(path).await;
    }
    let parent = path
        .parent()
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
    canonical_existing_path(parent).await
}

async fn canonical_existing_path(path: &Path) -> AgentResult<PathBuf> {
    tokio::fs::canonicalize(path)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))
}

pub(super) fn validate_container_path(value: &str) -> AgentResult<()> {
    if value.is_empty() || value.contains("..") || !value.starts_with('/') || value.len() > 255 {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ));
    }
    Ok(())
}

pub(super) fn validate_relative_service_file_path(value: &str) -> AgentResult<PathBuf> {
    if value.is_empty()
        || value.len() > MAX_RELATIVE_PATH_BYTES
        || value.as_bytes().contains(&0)
        || value.as_bytes().iter().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.'))
        })
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ));
    }

    let path = Path::new(value);
    if path.is_absolute() {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ));
    }

    let mut has_component = false;
    for component in path.components() {
        match component {
            Component::Normal(_) => has_component = true,
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::InvalidPath,
                ));
            }
        }
    }

    if !has_component {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ));
    }

    Ok(path.to_path_buf())
}

pub(super) fn validate_file_contents(value: &str, max_bytes: usize) -> AgentResult<()> {
    if value.len() > max_bytes || value.as_bytes().contains(&0) {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ));
    }
    Ok(())
}

/// Validation keeps this identifier safe for fixed transport use. The control plane treats
/// `secret_ref` as an opaque identifier; namespace canonicalization is enforced by
/// central Hephaestus rather than the local agent.
pub(super) fn validate_secret_ref(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.'))
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidIdentity,
        ));
    }
    Ok(())
}

pub(super) fn validate_local_http_target(value: &str) -> AgentResult<()> {
    let url = url::Url::parse(value)
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidUrl))?;
    if url.scheme() != "http" || !matches!(url.host_str(), Some("127.0.0.1" | "::1")) {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    }
    Ok(())
}

pub(super) fn validate_local_tcp_target(value: &str) -> AgentResult<()> {
    let Some((host, port)) = value.rsplit_once(':') else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    };
    if !matches!(host, "127.0.0.1" | "::1") {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    }
    let parsed = port
        .parse::<u16>()
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidUrl))?;
    if parsed == 0 {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_identifier, validate_secret_ref};
    use crate::docker_service::DockerServiceSpec;
    use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1 as pb;

    #[test]
    fn opaque_secret_references_preserve_controller_owned_names() {
        for value in [".", "..", "-credential", "team/service/key"] {
            assert!(validate_secret_ref(value).is_ok());
        }
    }

    #[test]
    fn identifiers_cannot_escape_service_roots_or_become_options() {
        for value in [".", "..", "../nats", "/etc", "-nats", "--help", ""] {
            assert!(validate_identifier(value).is_err(), "accepted {value:?}");
            assert!(
                DockerServiceSpec::from_proto(pb::AgentDockerServiceAction {
                    service_id: value.to_owned(),
                    container_name: "nats".to_owned(),
                    image_ref: "nats:2".to_owned(),
                    ..Default::default()
                })
                .is_err()
            );
        }
        for value in ["nats", "reallyme-nats", "worker_1", "api.v2"] {
            assert!(validate_identifier(value).is_ok());
        }
    }
}
