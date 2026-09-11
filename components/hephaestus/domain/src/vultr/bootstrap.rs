// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Strong provisioning bootstrap types for Vultr-created instances.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use super::VultrError;

const MAX_DOCKER_IMAGE_REFERENCE_BYTES: usize = 256;
const MAX_DOCKER_CONTAINER_NAME_BYTES: usize = 64;
const MAX_DOCKER_OPTION_BYTES: usize = 256;
const MAX_SSH_AUTHORIZED_KEY_BYTES: usize = 1024;

/// Validated Docker image reference used in generated bootstrap scripts.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct DockerImageReference(String);

impl DockerImageReference {
    /// Constructs a validated Docker image reference.
    pub fn new(value: impl Into<String>) -> Result<Self, VultrError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(VultrError::Empty);
        }
        if trimmed.len() > MAX_DOCKER_IMAGE_REFERENCE_BYTES {
            return Err(VultrError::TooLong);
        }
        if !trimmed.bytes().all(is_docker_image_byte) {
            return Err(VultrError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated image reference.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for DockerImageReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DockerImageReference")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for DockerImageReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Validated Docker container name.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct DockerContainerName(String);

impl DockerContainerName {
    /// Constructs a validated Docker container name.
    pub fn new(value: impl Into<String>) -> Result<Self, VultrError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(VultrError::Empty);
        }
        if trimmed.len() > MAX_DOCKER_CONTAINER_NAME_BYTES {
            return Err(VultrError::TooLong);
        }
        if !trimmed.bytes().all(is_container_name_byte) {
            return Err(VultrError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated container name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for DockerContainerName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DockerContainerName")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for DockerContainerName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Validated SSH public key line for cloud-init authorized_keys.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct SshAuthorizedKey(String);

impl SshAuthorizedKey {
    /// Constructs a validated SSH public key line.
    pub fn new(value: impl Into<String>) -> Result<Self, VultrError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(VultrError::Empty);
        }
        if trimmed.len() > MAX_SSH_AUTHORIZED_KEY_BYTES {
            return Err(VultrError::TooLong);
        }
        if !trimmed.bytes().all(is_ssh_authorized_key_byte) {
            return Err(VultrError::InvalidCharacter);
        }
        if !trimmed.starts_with("ssh-ed25519 ")
            && !trimmed.starts_with("ssh-rsa ")
            && !trimmed.starts_with("ecdsa-sha2-nistp256 ")
        {
            return Err(VultrError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated key line.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for SshAuthorizedKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SshAuthorizedKey")
            .field(&"<redacted>")
            .finish()
    }
}

/// Validated Docker CLI argument used as one argv item, never as shell text.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct DockerRunArgument(String);

impl DockerRunArgument {
    /// Constructs a validated Docker run argument.
    pub fn new(value: impl Into<String>) -> Result<Self, VultrError> {
        validated_bootstrap_value(value.into(), is_docker_argument_byte).map(Self)
    }

    /// Returns the validated argument.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for DockerRunArgument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DockerRunArgument")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for DockerRunArgument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Validated Docker volume mount in `host_path:container_path[:mode]` form.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct DockerVolumeMount(String);

impl DockerVolumeMount {
    /// Constructs a validated Docker volume mount.
    pub fn new(value: impl Into<String>) -> Result<Self, VultrError> {
        let value = validated_bootstrap_value(value.into(), is_path_mount_byte)?;
        if !value.contains(':') {
            return Err(VultrError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    /// Returns the validated mount.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for DockerVolumeMount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DockerVolumeMount")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for DockerVolumeMount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Validated Docker DNS resolver address.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct DockerDnsResolver(String);

impl DockerDnsResolver {
    /// Constructs a validated DNS resolver value.
    pub fn new(value: impl Into<String>) -> Result<Self, VultrError> {
        validated_bootstrap_value(value.into(), is_ip_address_byte).map(Self)
    }

    /// Returns the validated resolver.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for DockerDnsResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DockerDnsResolver")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for DockerDnsResolver {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Docker restart policy for generated app containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockerRestartPolicy {
    /// Restart unless explicitly stopped.
    UnlessStopped,
    /// Always restart the container.
    Always,
    /// Do not auto-restart.
    No,
}

impl DockerRestartPolicy {
    /// Returns the Docker CLI value.
    pub const fn as_cli_value(self) -> &'static str {
        match self {
            Self::UnlessStopped => "unless-stopped",
            Self::Always => "always",
            Self::No => "no",
        }
    }
}

/// Node metadata carried from Hephaestus into `/etc/hephaestus/server.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusServerMetadata {
    dc: String,
    datacenters: Vec<String>,
    server_id: String,
    private_ip: Option<String>,
    extra_private_ips: Vec<String>,
    service_name: String,
    docker_name: String,
    disable_dns: bool,
    firewall: bool,
    ports: Vec<String>,
    cluster_id: String,
    role: String,
}

impl HephaestusServerMetadata {
    /// Constructs validated node metadata for the current Hephaestus
    /// bootstrap shape used by cloud-init and the local node agent.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dc: impl Into<String>,
        datacenters: Vec<String>,
        server_id: impl Into<String>,
        private_ip: Option<String>,
        extra_private_ips: Vec<String>,
        service_name: impl Into<String>,
        docker_name: impl Into<String>,
        disable_dns: bool,
        firewall: bool,
        ports: Vec<String>,
        cluster_id: impl Into<String>,
        role: impl Into<String>,
    ) -> Result<Self, VultrError> {
        Ok(Self {
            dc: validated_metadata_value(dc.into())?,
            datacenters: validate_metadata_values(datacenters)?,
            server_id: validated_metadata_value(server_id.into())?,
            private_ip: validate_optional_metadata_value(private_ip)?,
            extra_private_ips: validate_metadata_values(extra_private_ips)?,
            service_name: validated_metadata_value(service_name.into())?,
            docker_name: validated_metadata_value(docker_name.into())?,
            disable_dns,
            firewall,
            ports: validate_metadata_values(ports)?,
            cluster_id: validated_metadata_value(cluster_id.into())?,
            role: validated_metadata_value(role.into())?,
        })
    }

    /// Returns the datacenter code.
    pub fn dc(&self) -> &str {
        self.dc.as_str()
    }

    /// Returns the server identifier.
    pub fn server_id(&self) -> &str {
        self.server_id.as_str()
    }

    /// Returns the private IP when one was reserved ahead of create.
    pub fn private_ip(&self) -> Option<&str> {
        self.private_ip.as_deref()
    }

    /// Returns container ports exposed to external traffic.
    pub fn ports(&self) -> &[String] {
        self.ports.as_slice()
    }
}

/// Bootstrap settings for the small Hephaestus node agent container.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct HephaestusAgentCloudInitSpec {
    image: DockerImageReference,
    container_name: DockerContainerName,
    callback_base_url: crate::HephaestusControllerBaseUrl,
    boot_secret: crate::HephaestusAgentBootSecret,
    control_secret: crate::HephaestusAgentControlSecret,
}

impl HephaestusAgentCloudInitSpec {
    /// Constructs a Hephaestus agent bootstrap spec.
    pub const fn new(
        image: DockerImageReference,
        container_name: DockerContainerName,
        callback_base_url: crate::HephaestusControllerBaseUrl,
        boot_secret: crate::HephaestusAgentBootSecret,
        control_secret: crate::HephaestusAgentControlSecret,
    ) -> Self {
        Self {
            image,
            container_name,
            callback_base_url,
            boot_secret,
            control_secret,
        }
    }

    /// Returns the Docker image.
    pub const fn image(&self) -> &DockerImageReference {
        &self.image
    }

    /// Returns the Docker container name.
    pub const fn container_name(&self) -> &DockerContainerName {
        &self.container_name
    }

    /// Returns the Hephaestus controller callback base URL.
    pub const fn callback_base_url(&self) -> &crate::HephaestusControllerBaseUrl {
        &self.callback_base_url
    }

    /// Returns the redacted one-time boot secret.
    pub const fn boot_secret(&self) -> &crate::HephaestusAgentBootSecret {
        &self.boot_secret
    }

    /// Returns the redacted shared control secret.
    pub const fn control_secret(&self) -> &crate::HephaestusAgentControlSecret {
        &self.control_secret
    }
}

impl fmt::Debug for HephaestusAgentCloudInitSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HephaestusAgentCloudInitSpec")
            .field("image", &self.image)
            .field("container_name", &self.container_name)
            .field("callback_base_url", &self.callback_base_url)
            .field("boot_secret", &"<redacted>")
            .field("control_secret", &"<redacted>")
            .finish()
    }
}

/// App bootstrap input for a single Docker container on a new VPS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DockerCloudInitSpec {
    image: DockerImageReference,
    container_name: DockerContainerName,
    restart_policy: DockerRestartPolicy,
    host_network: bool,
    ssh_authorized_key: Option<SshAuthorizedKey>,
    hephaestus_agent: Option<HephaestusAgentCloudInitSpec>,
    dns_resolvers: Vec<DockerDnsResolver>,
    volumes: Vec<DockerVolumeMount>,
    arguments: Vec<DockerRunArgument>,
    metadata: Option<HephaestusServerMetadata>,
}

impl DockerCloudInitSpec {
    /// Constructs a Docker cloud-init specification.
    pub const fn new(
        image: DockerImageReference,
        container_name: DockerContainerName,
        restart_policy: DockerRestartPolicy,
        host_network: bool,
    ) -> Self {
        Self {
            image,
            container_name,
            restart_policy,
            host_network,
            ssh_authorized_key: None,
            hephaestus_agent: None,
            dns_resolvers: Vec::new(),
            volumes: Vec::new(),
            arguments: Vec::new(),
            metadata: None,
        }
    }

    /// Returns the Docker image.
    pub const fn image(&self) -> &DockerImageReference {
        &self.image
    }

    /// Returns the container name.
    pub const fn container_name(&self) -> &DockerContainerName {
        &self.container_name
    }

    /// Returns the restart policy.
    pub const fn restart_policy(&self) -> DockerRestartPolicy {
        self.restart_policy
    }

    /// Returns whether Docker host networking should be used.
    pub const fn host_network(&self) -> bool {
        self.host_network
    }

    /// Returns the optional SSH authorized key.
    pub const fn ssh_authorized_key(&self) -> Option<&SshAuthorizedKey> {
        self.ssh_authorized_key.as_ref()
    }

    /// Returns the optional Hephaestus agent bootstrap spec.
    pub const fn hephaestus_agent(&self) -> Option<&HephaestusAgentCloudInitSpec> {
        self.hephaestus_agent.as_ref()
    }

    /// Returns Docker DNS resolvers.
    pub fn dns_resolvers(&self) -> &[DockerDnsResolver] {
        self.dns_resolvers.as_slice()
    }

    /// Returns Docker volume mounts.
    pub fn volumes(&self) -> &[DockerVolumeMount] {
        self.volumes.as_slice()
    }

    /// Returns app command arguments.
    pub fn arguments(&self) -> &[DockerRunArgument] {
        self.arguments.as_slice()
    }

    /// Returns optional Hephaestus node metadata.
    pub const fn metadata(&self) -> Option<&HephaestusServerMetadata> {
        self.metadata.as_ref()
    }

    /// Returns a copy of this spec with a root SSH authorized key.
    pub fn with_ssh_authorized_key(mut self, key: SshAuthorizedKey) -> Self {
        self.ssh_authorized_key = Some(key);
        self
    }

    /// Returns a copy of this spec with the Hephaestus agent enabled.
    pub fn with_hephaestus_agent(mut self, agent: HephaestusAgentCloudInitSpec) -> Self {
        self.hephaestus_agent = Some(agent);
        self
    }

    /// Returns a copy of this spec with Docker DNS resolvers.
    pub fn with_dns_resolvers(mut self, dns_resolvers: Vec<DockerDnsResolver>) -> Self {
        self.dns_resolvers = dns_resolvers;
        self
    }

    /// Returns a copy of this spec with Docker volume mounts.
    pub fn with_volumes(mut self, volumes: Vec<DockerVolumeMount>) -> Self {
        self.volumes = volumes;
        self
    }

    /// Returns a copy of this spec with app command arguments.
    pub fn with_arguments(mut self, arguments: Vec<DockerRunArgument>) -> Self {
        self.arguments = arguments;
        self
    }

    /// Returns a copy of this spec with Hephaestus node metadata.
    pub fn with_metadata(mut self, metadata: HephaestusServerMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

fn validated_bootstrap_value(value: String, allow: fn(u8) -> bool) -> Result<String, VultrError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(VultrError::Empty);
    }
    if trimmed.len() > MAX_DOCKER_OPTION_BYTES {
        return Err(VultrError::TooLong);
    }
    if !trimmed.bytes().all(allow) {
        return Err(VultrError::InvalidCharacter);
    }
    Ok(trimmed.to_owned())
}

fn validated_metadata_value(value: String) -> Result<String, VultrError> {
    validated_bootstrap_value(value, is_metadata_value_byte)
}

fn validate_optional_metadata_value(value: Option<String>) -> Result<Option<String>, VultrError> {
    value.map(validated_metadata_value).transpose()
}

fn validate_metadata_values(values: Vec<String>) -> Result<Vec<String>, VultrError> {
    values.into_iter().map(validated_metadata_value).collect()
}

fn is_docker_image_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/' | b':' | b'@' | b'+')
}

fn is_container_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
}

fn is_docker_argument_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'.' | b'-' | b'_' | b'/' | b':' | b'@' | b'+' | b'=' | b',' | b'?' | b'&'
        )
}

fn is_path_mount_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/' | b':' | b'=')
}

fn is_ip_address_byte(byte: u8) -> bool {
    byte.is_ascii_hexdigit() || matches!(byte, b'.' | b':')
}

fn is_ssh_authorized_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'+' | b'/' | b'=' | b'-' | b'_')
}

fn is_metadata_value_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'.' | b'-' | b'_' | b'/' | b':' | b'@' | b'+' | b'=' | b',' | b' '
        )
}

#[cfg(test)]
mod tests {
    use super::{DockerImageReference, DockerVolumeMount, SshAuthorizedKey};
    use crate::vultr::VultrError;

    #[test]
    fn docker_image_reference_rejects_shell_metacharacters() {
        assert_eq!(
            DockerImageReference::new("ghcr.io/reallyme/app:latest;rm -rf /"),
            Err(VultrError::InvalidCharacter)
        );
    }

    #[test]
    fn ssh_authorized_key_requires_known_key_prefix() {
        assert_eq!(
            SshAuthorizedKey::new("not-a-key AAAAC3NzaC1lZDI1NTE5AAAAIFixtureKey"),
            Err(VultrError::InvalidCharacter)
        );
    }

    #[test]
    fn docker_volume_mount_requires_host_and_container_path() {
        assert_eq!(
            DockerVolumeMount::new("/root/var/lib/app"),
            Err(VultrError::InvalidCharacter)
        );
    }
}
