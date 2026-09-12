// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic, least-privilege Compose rendering for managed services.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;

use super::{
    AgentResult, CONTAINER_CONFIG_DIR, CONTAINER_SECRET_DIR, DockerDevice, DockerPort,
    DockerServiceSpec, DockerVolume, EnvironmentVariable, HephaestusAgentError,
    HephaestusAgentErrorReason, LinuxCapability, NetworkMode, PortProtocol, config_dir, secret_dir,
};

pub(super) fn render(spec: &DockerServiceSpec) -> AgentResult<String> {
    serde_norway::to_string(&ComposeFile::from_spec(spec))
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cap_add: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    devices: Vec<String>,
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
            network_mode: runtime_network_mode(spec).map(str::to_owned),
            read_only: spec.read_only_root_filesystem
                || production_read_only_rootfs(spec.service_id()),
            tmpfs: production_tmpfs_mounts(spec.service_id()),
            security_opt: runtime_security_options(spec),
            cap_drop: runtime_cap_drop(spec),
            cap_add: render_linux_capabilities(spec.linux_capabilities.as_slice()),
            devices: render_devices(spec.devices.as_slice()),
            pids_limit: production_pids_limit(spec.service_id()),
            healthcheck: production_healthcheck(spec.service_id()),
            environment: environment_map(spec.environment.as_slice()),
            ports: runtime_network_mode(spec)
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

fn runtime_network_mode(spec: &DockerServiceSpec) -> Option<&'static str> {
    match spec.network_mode {
        NetworkMode::Default => production_network_mode(spec.service_id()),
        NetworkMode::Host => Some("host"),
        NetworkMode::Bridge => None,
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

fn runtime_security_options(spec: &DockerServiceSpec) -> Vec<String> {
    if spec.no_new_privileges || is_hardened_production_service(spec.service_id()) {
        vec![String::from("no-new-privileges:true")]
    } else {
        Vec::new()
    }
}

fn runtime_cap_drop(spec: &DockerServiceSpec) -> Vec<String> {
    if has_runtime_authority(spec)
        || matches!(
            spec.service_id(),
            "reallyme-nats" | "reallyme-typesense" | "reallyme-web"
        )
    {
        vec![String::from("ALL")]
    } else {
        Vec::new()
    }
}

fn has_runtime_authority(spec: &DockerServiceSpec) -> bool {
    !spec.devices.is_empty() || !spec.linux_capabilities.is_empty()
}

fn is_hardened_production_service(service_id: &str) -> bool {
    matches!(
        service_id,
        "reallyme-caddy" | "reallyme-nats" | "reallyme-typesense" | "reallyme-web"
    )
}

fn render_linux_capabilities(capabilities: &[LinuxCapability]) -> Vec<String> {
    capabilities
        .iter()
        .map(|capability| match capability {
            LinuxCapability::NetAdmin => "NET_ADMIN",
            LinuxCapability::NetBindService => "NET_BIND_SERVICE",
        })
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn render_devices(devices: &[DockerDevice]) -> Vec<String> {
    devices
        .iter()
        .map(|device| match device {
            DockerDevice::Tun => "/dev/net/tun:/dev/net/tun:rwm",
        })
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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
