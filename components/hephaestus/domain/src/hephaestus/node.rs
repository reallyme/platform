// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Boot-report and actual-node-state types for Hephaestus-managed hosts.

use std::fmt;
use std::net::IpAddr;

use serde::{Deserialize, Deserializer, Serialize};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{HephaestusDomainError, HephaestusSiteId};

const MAX_CONTROLLER_BASE_URL_BYTES: usize = 2_048;
const MAX_AGENT_BOOT_SECRET_BYTES: usize = 128;
const MAX_AGENT_CONTROL_SECRET_BYTES: usize = 128;
const MAX_AGENT_RUNTIME_TOKEN_BYTES: usize = 128;
const AGENT_ED25519_PUBLIC_KEY_BASE64URL_BYTES: usize = 43;
const AGENT_PUBLIC_KEY_FINGERPRINT_BYTES: usize = 64;
const MAX_SERVER_ID_BYTES: usize = 64;
const MAX_PROVIDER_SERVER_ID_BYTES: usize = 128;
const MAX_CLUSTER_ID_BYTES: usize = 64;
const MAX_INFRASTRUCTURE_CLASS_ID_BYTES: usize = 128;
const MAX_SERVER_APP_BYTES: usize = 64;
const MAX_SERVER_SERVICE_BYTES: usize = 128;
const MAX_SERVER_APPS: usize = 32;
const MAX_SERVER_SERVICES: usize = 64;

/// Validated Hephaestus controller base URL used by node agents.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusControllerBaseUrl(String);

impl HephaestusControllerBaseUrl {
    /// Constructs a validated controller base URL.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        validate_controller_base_url(value.as_str())?;
        Ok(Self(value))
    }

    /// Returns the validated controller base URL.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for HephaestusControllerBaseUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusControllerBaseUrl")
            .field(&self.0)
            .finish()
    }
}

/// Validated base URL for the infrastructure metadata service.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusMetadataBaseUrl(String);

impl HephaestusMetadataBaseUrl {
    /// Constructs a validated metadata service base URL.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        validate_controller_base_url(value.as_str())?;
        Ok(Self(value))
    }

    /// Returns the validated metadata base URL.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for HephaestusMetadataBaseUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusMetadataBaseUrl")
            .field(&self.0)
            .finish()
    }
}

/// Typed boot secret injected into node bootstrap metadata and used once the
/// node phones home.
#[derive(Clone, PartialEq, Eq, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(transparent)]
pub struct HephaestusAgentBootSecret(String);

impl HephaestusAgentBootSecret {
    /// Constructs a validated boot secret.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_AGENT_BOOT_SECRET_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_boot_secret_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the boot secret as a string.
    pub fn expose_as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Compares two boot secrets in constant time.
    ///
    /// Authentication paths must use this method instead of `==`/`!=`. The
    /// derived `PartialEq` runs `String` byte equality, which short-circuits on
    /// the first differing byte and leaks a timing side channel that lets a
    /// remote attacker recover the secret byte-by-byte.
    pub fn constant_time_eq(&self, other: &Self) -> bool {
        self.0.as_bytes().ct_eq(other.0.as_bytes()).into()
    }
}

/// Typed shared control secret used for node-local Hephaestus agent operations.
#[derive(Clone, PartialEq, Eq, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(transparent)]
pub struct HephaestusAgentControlSecret(String);

impl HephaestusAgentControlSecret {
    /// Constructs a validated control secret.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_AGENT_CONTROL_SECRET_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_boot_secret_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the control secret as a string.
    pub fn expose_as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Compares two control secrets in constant time.
    ///
    /// Authentication paths must use this method instead of `==`/`!=`. The
    /// derived `PartialEq` runs `String` byte equality, which short-circuits on
    /// the first differing byte and leaks a timing side channel that lets a
    /// remote attacker recover the secret byte-by-byte.
    pub fn constant_time_eq(&self, other: &Self) -> bool {
        self.0.as_bytes().ct_eq(other.0.as_bytes()).into()
    }
}

impl fmt::Debug for HephaestusAgentControlSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusAgentControlSecret")
            .field(&"<redacted>")
            .finish()
    }
}

/// Typed runtime token returned to a registered node agent.
///
/// Runtime tokens replace the one-time bootstrap token after first boot. They
/// authorize the node to report health and poll actions for its own `node_id`;
/// they must not be stored in cloud-init config or golden images.
#[derive(Clone, PartialEq, Eq, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(transparent)]
pub struct HephaestusAgentRuntimeToken(String);

impl HephaestusAgentRuntimeToken {
    /// Constructs a validated runtime token.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_AGENT_RUNTIME_TOKEN_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_boot_secret_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the runtime token as a string.
    pub fn expose_as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Compares two runtime tokens in constant time.
    pub fn constant_time_eq(&self, other: &Self) -> bool {
        self.0.as_bytes().ct_eq(other.0.as_bytes()).into()
    }
}

impl fmt::Debug for HephaestusAgentRuntimeToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusAgentRuntimeToken")
            .field(&"<redacted>")
            .finish()
    }
}

impl<'de> Deserialize<'de> for HephaestusAgentRuntimeToken {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Base64url-no-padding Ed25519 public key generated by a node agent.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusAgentPublicKey(String);

impl HephaestusAgentPublicKey {
    /// Constructs a validated node-agent public key.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.len() != AGENT_ED25519_PUBLIC_KEY_BASE64URL_BYTES {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        if !trimmed.bytes().all(is_base64url_no_pad_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the encoded Ed25519 public key.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for HephaestusAgentPublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusAgentPublicKey")
            .field(&self.0)
            .finish()
    }
}

/// SHA-256 hex fingerprint of a registered node-agent public key.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusAgentPublicKeyFingerprint(String);

impl HephaestusAgentPublicKeyFingerprint {
    /// Constructs a validated public-key fingerprint.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.len() != AGENT_PUBLIC_KEY_FINGERPRINT_BYTES {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        if !trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_ascii_lowercase()))
    }

    /// Returns the lowercase hex fingerprint.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Compares two public-key fingerprints in constant time.
    pub fn constant_time_eq(&self, other: &Self) -> bool {
        self.0.as_bytes().ct_eq(other.0.as_bytes()).into()
    }
}

impl fmt::Debug for HephaestusAgentPublicKeyFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusAgentPublicKeyFingerprint")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for HephaestusAgentControlSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

impl fmt::Debug for HephaestusAgentBootSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusAgentBootSecret")
            .field(&"<redacted>")
            .finish()
    }
}

impl<'de> Deserialize<'de> for HephaestusAgentBootSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Stable result of a submitted boot report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusAgentBootReportResult {
    /// The boot report matched the expected orchestrator state and was accepted.
    Accepted,
    /// Hephaestus had no matching expected boot state for the supplied server.
    RejectedUnknownServer,
    /// The submitted boot secret did not match the expected orchestrator state.
    RejectedAuthentication,
}

/// Actual node status derived from boot/heartbeat reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusNodeActualStateStatus {
    /// The node has successfully completed its boot report.
    Ready,
    /// The node has missed its freshness window and needs reconciliation.
    Stale,
    /// The node has been reconciled as stopped, destroyed, or unreachable.
    Offline,
}

/// Stable Hephaestus internal server identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusServerId(String);

impl HephaestusServerId {
    /// Constructs a validated internal server identity.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_SERVER_ID_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_server_identity_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated internal server identity.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Provider-backed infrastructure server identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusProviderServerId(String);

impl HephaestusProviderServerId {
    /// Constructs a validated provider-backed server identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_PROVIDER_SERVER_ID_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_server_identity_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the provider identifier text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Stable infrastructure class, shape, or size identifier for a server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusInfrastructureClassId(String);

impl HephaestusInfrastructureClassId {
    /// Constructs a validated infrastructure class identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_INFRASTRUCTURE_CLASS_ID_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_server_identity_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated infrastructure class identifier.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Public IP address observed for a managed server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusPublicIpAddress(String);

impl HephaestusPublicIpAddress {
    /// Constructs a validated public IP address.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        validate_ip_address(value.as_str())?;
        Ok(Self(value))
    }

    /// Returns the validated IP address.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Private IP address observed or planned for a managed server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusPrivateIpAddress(String);

impl HephaestusPrivateIpAddress {
    /// Constructs a validated private IP address.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        validate_ip_address(value.as_str())?;
        Ok(Self(value))
    }

    /// Returns the validated IP address.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Stable logical group identifier for a server pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusClusterId(String);

impl HephaestusClusterId {
    /// Constructs a validated cluster identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_CLUSTER_ID_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_server_identity_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated group identifier.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Broad machine purpose used for server inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusServerRole {
    /// Public or private application serving node.
    Api,
    /// Background job or queue worker.
    Worker,
    /// Database server.
    Db,
    /// Search/index server.
    Search,
    /// Edge or ingress server.
    Edge,
    /// Role is intentionally not yet classified.
    Unknown,
}

/// Typed app identity deployed onto one server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusServerApp(String);

impl HephaestusServerApp {
    /// Constructs a validated deployed-app identity.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_SERVER_APP_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_server_identity_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated app identity.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Typed service surface identity exposed by deployed apps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusServerService(String);

impl HephaestusServerService {
    /// Constructs a validated service surface identity.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_SERVER_SERVICE_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_server_service_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated service surface identity.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Stable network identity associated with a single server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusServerIdentity {
    server_id: HephaestusServerId,
    provider_server_id: HephaestusProviderServerId,
    site_id: HephaestusSiteId,
}

impl HephaestusServerIdentity {
    /// Constructs stable identity fields for a server.
    pub const fn new(
        server_id: HephaestusServerId,
        provider_server_id: HephaestusProviderServerId,
        site_id: HephaestusSiteId,
    ) -> Self {
        Self {
            server_id,
            provider_server_id,
            site_id,
        }
    }

    /// Returns the internal server identifier.
    pub const fn server_id(&self) -> &HephaestusServerId {
        &self.server_id
    }

    /// Returns the provider-backed server identifier.
    pub const fn provider_server_id(&self) -> &HephaestusProviderServerId {
        &self.provider_server_id
    }

    /// Returns the stable Hephaestus site identifier.
    pub const fn site_id(&self) -> &HephaestusSiteId {
        &self.site_id
    }
}

/// Observed node IP addresses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusServerNetwork {
    public_ip: Option<HephaestusPublicIpAddress>,
    private_ip: Option<HephaestusPrivateIpAddress>,
}

impl HephaestusServerNetwork {
    /// Constructs the observed network summary for a server.
    pub const fn new(
        public_ip: Option<HephaestusPublicIpAddress>,
        private_ip: Option<HephaestusPrivateIpAddress>,
    ) -> Self {
        Self {
            public_ip,
            private_ip,
        }
    }

    /// Returns the public IP when known.
    pub const fn public_ip(&self) -> Option<&HephaestusPublicIpAddress> {
        self.public_ip.as_ref()
    }

    /// Returns the private IP when known.
    pub const fn private_ip(&self) -> Option<&HephaestusPrivateIpAddress> {
        self.private_ip.as_ref()
    }
}

/// Boot-report timing observed on the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusServerBootTiming {
    boot_time_unix_secs: u64,
    generated_at_unix_secs: u64,
}

impl HephaestusServerBootTiming {
    /// Constructs validated boot-report timing values.
    pub fn new(
        boot_time_unix_secs: u64,
        generated_at_unix_secs: u64,
    ) -> Result<Self, HephaestusDomainError> {
        if boot_time_unix_secs == 0
            || generated_at_unix_secs == 0
            || boot_time_unix_secs > generated_at_unix_secs
        {
            return Err(HephaestusDomainError::InvalidNumber);
        }
        Ok(Self {
            boot_time_unix_secs,
            generated_at_unix_secs,
        })
    }

    /// Returns the observed boot time.
    pub const fn boot_time_unix_secs(&self) -> u64 {
        self.boot_time_unix_secs
    }

    /// Returns the report emission time.
    pub const fn generated_at_unix_secs(&self) -> u64 {
        self.generated_at_unix_secs
    }
}

/// One authenticated boot report emitted by the node agent after first boot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusAgentBootReport {
    identity: HephaestusServerIdentity,
    network: HephaestusServerNetwork,
    timing: HephaestusServerBootTiming,
}

impl HephaestusAgentBootReport {
    /// Constructs a validated boot report.
    pub fn new(
        identity: HephaestusServerIdentity,
        network: HephaestusServerNetwork,
        timing: HephaestusServerBootTiming,
    ) -> Result<Self, HephaestusDomainError> {
        Ok(Self {
            identity,
            network,
            timing,
        })
    }

    /// Returns the internal server identifier.
    pub const fn server_id(&self) -> &HephaestusServerId {
        self.identity.server_id()
    }

    /// Returns the provider-backed server identifier.
    pub const fn provider_server_id(&self) -> &HephaestusProviderServerId {
        self.identity.provider_server_id()
    }

    /// Returns the stable Hephaestus site identifier.
    pub const fn site_id(&self) -> &HephaestusSiteId {
        self.identity.site_id()
    }

    /// Returns the public IP when known.
    pub const fn public_ip(&self) -> Option<&HephaestusPublicIpAddress> {
        self.network.public_ip()
    }

    /// Returns the private IP when known.
    pub const fn private_ip(&self) -> Option<&HephaestusPrivateIpAddress> {
        self.network.private_ip()
    }

    /// Returns the observed boot time.
    pub const fn boot_time_unix_secs(&self) -> u64 {
        self.timing.boot_time_unix_secs()
    }

    /// Returns the time at which the report was emitted.
    pub const fn generated_at_unix_secs(&self) -> u64 {
        self.timing.generated_at_unix_secs()
    }
}

/// Durable actual-state summary used by the dashboard and readiness logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusNodeActualState {
    identity: HephaestusServerIdentity,
    network: HephaestusServerNetwork,
    catalog: HephaestusServerCatalog,
    infrastructure_class_id: HephaestusInfrastructureClassId,
    agent_public_key: Option<HephaestusAgentPublicKey>,
    agent_public_key_fingerprint: Option<HephaestusAgentPublicKeyFingerprint>,
    boot_time_unix_secs: u64,
    last_seen_unix_secs: u64,
    status: HephaestusNodeActualStateStatus,
}

impl HephaestusNodeActualState {
    /// Constructs a validated actual-state summary.
    pub fn new(
        identity: HephaestusServerIdentity,
        network: HephaestusServerNetwork,
        catalog: HephaestusServerCatalog,
        infrastructure_class_id: HephaestusInfrastructureClassId,
        boot_time_unix_secs: u64,
        last_seen_unix_secs: u64,
        status: HephaestusNodeActualStateStatus,
    ) -> Result<Self, HephaestusDomainError> {
        if boot_time_unix_secs == 0
            || last_seen_unix_secs == 0
            || boot_time_unix_secs > last_seen_unix_secs
        {
            return Err(HephaestusDomainError::InvalidNumber);
        }
        Ok(Self {
            identity,
            network,
            catalog,
            infrastructure_class_id,
            agent_public_key: None,
            agent_public_key_fingerprint: None,
            boot_time_unix_secs,
            last_seen_unix_secs,
            status,
        })
    }

    /// Returns a copy with the node-local agent public identity attached.
    pub fn with_agent_public_identity(
        mut self,
        public_key: HephaestusAgentPublicKey,
        fingerprint: HephaestusAgentPublicKeyFingerprint,
    ) -> Self {
        self.agent_public_key = Some(public_key);
        self.agent_public_key_fingerprint = Some(fingerprint);
        self
    }

    /// Constructs an actual-state record from an accepted boot report.
    pub fn from_boot_report(
        report: &HephaestusAgentBootReport,
        orchestrator_state: &HephaestusOrchestratorNodeState,
        status: HephaestusNodeActualStateStatus,
    ) -> Result<Self, HephaestusDomainError> {
        Self::new(
            report.identity.clone(),
            report.network.clone(),
            orchestrator_state.catalog().clone(),
            orchestrator_state.infrastructure_class_id().clone(),
            report.boot_time_unix_secs(),
            report.generated_at_unix_secs(),
            status,
        )
    }

    /// Returns a copy with an updated last-seen timestamp.
    pub fn with_last_seen_unix_secs(
        mut self,
        last_seen_unix_secs: u64,
    ) -> Result<Self, HephaestusDomainError> {
        if last_seen_unix_secs == 0 || last_seen_unix_secs < self.boot_time_unix_secs {
            return Err(HephaestusDomainError::InvalidNumber);
        }
        self.last_seen_unix_secs = last_seen_unix_secs;
        Ok(self)
    }

    /// Returns a copy with an updated actual-state status.
    pub const fn with_status(mut self, status: HephaestusNodeActualStateStatus) -> Self {
        self.status = status;
        self
    }

    /// Returns the internal server identifier.
    pub const fn server_id(&self) -> &HephaestusServerId {
        self.identity.server_id()
    }

    /// Returns the full server identity.
    pub const fn identity(&self) -> &HephaestusServerIdentity {
        &self.identity
    }

    /// Returns the provider-backed server identifier.
    pub const fn provider_server_id(&self) -> &HephaestusProviderServerId {
        self.identity.provider_server_id()
    }

    /// Returns the stable Hephaestus site identifier.
    pub const fn site_id(&self) -> &HephaestusSiteId {
        self.identity.site_id()
    }

    /// Returns the infrastructure class identifier recorded for this server.
    pub const fn infrastructure_class_id(&self) -> &HephaestusInfrastructureClassId {
        &self.infrastructure_class_id
    }

    /// Returns the public IP when known.
    pub const fn public_ip(&self) -> Option<&HephaestusPublicIpAddress> {
        self.network.public_ip()
    }

    /// Returns the private IP when known.
    pub const fn private_ip(&self) -> Option<&HephaestusPrivateIpAddress> {
        self.network.private_ip()
    }

    /// Returns the registered node-agent public key when the agent enrolled.
    pub const fn agent_public_key(&self) -> Option<&HephaestusAgentPublicKey> {
        self.agent_public_key.as_ref()
    }

    /// Returns the registered node-agent public-key fingerprint when enrolled.
    pub const fn agent_public_key_fingerprint(
        &self,
    ) -> Option<&HephaestusAgentPublicKeyFingerprint> {
        self.agent_public_key_fingerprint.as_ref()
    }

    /// Returns the logical group identifier when known.
    pub const fn cluster_id(&self) -> Option<&HephaestusClusterId> {
        self.catalog.cluster_id()
    }

    /// Returns the full server catalog.
    pub const fn catalog(&self) -> &HephaestusServerCatalog {
        &self.catalog
    }

    /// Returns the broad server role.
    pub const fn role(&self) -> HephaestusServerRole {
        self.catalog.role()
    }

    /// Returns deployed apps.
    pub fn apps(&self) -> &[HephaestusServerApp] {
        self.catalog.apps()
    }

    /// Returns exposed service surfaces.
    pub fn services(&self) -> &[HephaestusServerService] {
        self.catalog.services()
    }

    /// Returns the observed boot time.
    pub const fn boot_time_unix_secs(&self) -> u64 {
        self.boot_time_unix_secs
    }

    /// Returns the last successful contact time.
    pub const fn last_seen_unix_secs(&self) -> u64 {
        self.last_seen_unix_secs
    }

    /// Returns the current actual-state status.
    pub const fn status(&self) -> HephaestusNodeActualStateStatus {
        self.status
    }
}

/// Expected node bootstrap state recorded by the control plane before boot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusOrchestratorNodeState {
    identity: HephaestusServerIdentity,
    catalog: HephaestusServerCatalog,
    infrastructure_class_id: HephaestusInfrastructureClassId,
    expected_private_ip: Option<HephaestusPrivateIpAddress>,
    boot_secret: HephaestusAgentBootSecret,
}

impl HephaestusOrchestratorNodeState {
    /// Constructs expected node bootstrap state.
    pub const fn new(
        identity: HephaestusServerIdentity,
        catalog: HephaestusServerCatalog,
        infrastructure_class_id: HephaestusInfrastructureClassId,
        expected_private_ip: Option<HephaestusPrivateIpAddress>,
        boot_secret: HephaestusAgentBootSecret,
    ) -> Self {
        Self {
            identity,
            catalog,
            infrastructure_class_id,
            expected_private_ip,
            boot_secret,
        }
    }

    /// Returns the internal server identifier.
    pub const fn server_id(&self) -> &HephaestusServerId {
        self.identity.server_id()
    }

    /// Returns the full server identity.
    pub const fn identity(&self) -> &HephaestusServerIdentity {
        &self.identity
    }

    /// Returns the provider-backed server identifier.
    pub const fn provider_server_id(&self) -> &HephaestusProviderServerId {
        self.identity.provider_server_id()
    }

    /// Returns the expected Hephaestus site identifier.
    pub const fn site_id(&self) -> &HephaestusSiteId {
        self.identity.site_id()
    }

    /// Returns the infrastructure class identifier recorded for this server.
    pub const fn infrastructure_class_id(&self) -> &HephaestusInfrastructureClassId {
        &self.infrastructure_class_id
    }

    /// Returns the expected private IP when reserved ahead of create.
    pub const fn expected_private_ip(&self) -> Option<&HephaestusPrivateIpAddress> {
        self.expected_private_ip.as_ref()
    }

    /// Returns the planned server catalog metadata.
    pub const fn catalog(&self) -> &HephaestusServerCatalog {
        &self.catalog
    }

    /// Returns the logical cluster when one is assigned.
    pub const fn cluster_id(&self) -> Option<&HephaestusClusterId> {
        self.catalog.cluster_id()
    }

    /// Returns the broad planned role.
    pub const fn role(&self) -> HephaestusServerRole {
        self.catalog.role()
    }

    /// Returns the planned app list.
    pub fn apps(&self) -> &[HephaestusServerApp] {
        self.catalog.apps()
    }

    /// Returns the planned service list.
    pub fn services(&self) -> &[HephaestusServerService] {
        self.catalog.services()
    }

    /// Returns the redacted boot secret.
    pub const fn boot_secret(&self) -> &HephaestusAgentBootSecret {
        &self.boot_secret
    }
}

fn validate_ip_address(value: &str) -> Result<(), HephaestusDomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(HephaestusDomainError::Empty);
    }
    trimmed
        .parse::<IpAddr>()
        .map(|_| ())
        .map_err(|_error| HephaestusDomainError::InvalidCharacter)
}

fn validate_controller_base_url(value: &str) -> Result<(), HephaestusDomainError> {
    if value.is_empty() {
        return Err(HephaestusDomainError::Empty);
    }
    if value.len() > MAX_CONTROLLER_BASE_URL_BYTES {
        return Err(HephaestusDomainError::TooLong);
    }
    if value.chars().any(char::is_whitespace) {
        return Err(HephaestusDomainError::InvalidCharacter);
    }
    if value.contains('?') || value.contains('#') {
        return Err(HephaestusDomainError::InvalidCharacter);
    }

    let without_scheme = if let Some(value) = value.strip_prefix("https://") {
        value
    } else if let Some(value) = value.strip_prefix("http://") {
        value
    } else {
        return Err(HephaestusDomainError::InvalidCharacter);
    };

    let (authority, path) = without_scheme
        .split_once('/')
        .map_or((without_scheme, ""), |(authority, path)| (authority, path));

    if authority.is_empty() || authority.contains('@') || !path.is_empty() {
        return Err(HephaestusDomainError::InvalidCharacter);
    }

    Ok(())
}

fn is_boot_secret_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
}

fn is_base64url_no_pad_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

fn is_server_identity_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
}

fn is_server_service_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "fixed test fixtures should fail loudly if validation invariants change"
)]
mod tests {
    use super::{
        HephaestusAgentBootReport, HephaestusAgentBootSecret, HephaestusClusterId,
        HephaestusControllerBaseUrl, HephaestusInfrastructureClassId, HephaestusNodeActualState,
        HephaestusNodeActualStateStatus, HephaestusPrivateIpAddress, HephaestusProviderServerId,
        HephaestusServerApp, HephaestusServerBootTiming, HephaestusServerCatalog,
        HephaestusServerId, HephaestusServerIdentity, HephaestusServerNetwork,
        HephaestusServerRole, HephaestusServerService,
    };
    use crate::HephaestusSiteId;

    #[test]
    fn controller_base_url_rejects_paths() {
        assert!(HephaestusControllerBaseUrl::new("https://hephaestus.reallyme.net/path").is_err());
        assert!(HephaestusControllerBaseUrl::new("https://hephaestus.reallyme.net").is_ok());
    }

    #[test]
    fn boot_secret_debug_is_redacted() {
        let secret = HephaestusAgentBootSecret::new("secret-token").expect("valid secret");
        let debug = format!("{secret:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret-token"));
    }

    #[test]
    fn boot_report_builds_node_actual_state() {
        let report = HephaestusAgentBootReport::new(
            HephaestusServerIdentity::new(
                HephaestusServerId::new("srv-lhr-01").expect("valid server"),
                HephaestusProviderServerId::new("provider-server-1")
                    .expect("valid provider server"),
                HephaestusSiteId::new("lhr").expect("valid site"),
            ),
            HephaestusServerNetwork::new(
                None,
                Some(HephaestusPrivateIpAddress::new("10.1.100.10").expect("valid private ip")),
            ),
            HephaestusServerBootTiming::new(10, 20).expect("valid timing"),
        )
        .expect("valid report");
        let catalog = HephaestusServerCatalog::new(
            Some(HephaestusClusterId::new("POOL-API").expect("valid group")),
            HephaestusServerRole::Api,
            vec![HephaestusServerApp::new("api").expect("valid app")],
            vec![HephaestusServerService::new("api/http").expect("valid service")],
        )
        .expect("valid catalog");
        let orchestrator_state = super::HephaestusOrchestratorNodeState::new(
            HephaestusServerIdentity::new(
                HephaestusServerId::new("srv-lhr-01").expect("valid server"),
                HephaestusProviderServerId::new("provider-server-1")
                    .expect("valid provider server"),
                HephaestusSiteId::new("lhr").expect("valid site"),
            ),
            catalog,
            HephaestusInfrastructureClassId::new("standard-small")
                .expect("valid infrastructure class"),
            Some(HephaestusPrivateIpAddress::new("10.1.100.10").expect("valid private ip")),
            HephaestusAgentBootSecret::new("secret-token").expect("valid secret"),
        );

        let state = HephaestusNodeActualState::from_boot_report(
            &report,
            &orchestrator_state,
            HephaestusNodeActualStateStatus::Ready,
        )
        .expect("valid actual state");

        assert_eq!(state.last_seen_unix_secs(), 20);
        assert_eq!(state.status(), HephaestusNodeActualStateStatus::Ready);
        assert_eq!(state.server_id().as_str(), "srv-lhr-01");
    }
}
/// Desired server catalog information used by the dashboard and orchestration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusServerCatalog {
    cluster_id: Option<HephaestusClusterId>,
    role: HephaestusServerRole,
    apps: Vec<HephaestusServerApp>,
    services: Vec<HephaestusServerService>,
}

impl HephaestusServerCatalog {
    /// Constructs validated server catalog information.
    pub fn new(
        cluster_id: Option<HephaestusClusterId>,
        role: HephaestusServerRole,
        apps: Vec<HephaestusServerApp>,
        services: Vec<HephaestusServerService>,
    ) -> Result<Self, HephaestusDomainError> {
        if apps.len() > MAX_SERVER_APPS || services.len() > MAX_SERVER_SERVICES {
            return Err(HephaestusDomainError::TooLong);
        }
        Ok(Self {
            cluster_id,
            role,
            apps,
            services,
        })
    }

    /// Returns the logical group identifier when classified.
    pub const fn cluster_id(&self) -> Option<&HephaestusClusterId> {
        self.cluster_id.as_ref()
    }

    /// Returns the broad server role.
    pub const fn role(&self) -> HephaestusServerRole {
        self.role
    }

    /// Returns deployed app identities.
    pub fn apps(&self) -> &[HephaestusServerApp] {
        self.apps.as_slice()
    }

    /// Returns exposed service surface identities.
    pub fn services(&self) -> &[HephaestusServerService] {
        self.services.as_slice()
    }
}
