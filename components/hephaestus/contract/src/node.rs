// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(missing_docs)]

use reallyme_hephaestus_domain::{
    HephaestusAgentBootReport, HephaestusAgentBootReportResult, HephaestusAgentBootSecret,
    HephaestusAgentPublicKey, HephaestusAgentPublicKeyFingerprint, HephaestusAgentRuntimeToken,
    HephaestusNodeActualState, HephaestusServerId, HephaestusServerInventoryRow,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitAgentBootReportRequest {
    report: HephaestusAgentBootReport,
    boot_secret: HephaestusAgentBootSecret,
    agent_public_key: Option<HephaestusAgentPublicKey>,
    agent_public_key_fingerprint: Option<HephaestusAgentPublicKeyFingerprint>,
}

impl SubmitAgentBootReportRequest {
    /// Constructs a boot-report submission request.
    pub const fn new(
        report: HephaestusAgentBootReport,
        boot_secret: HephaestusAgentBootSecret,
    ) -> Self {
        Self {
            report,
            boot_secret,
            agent_public_key: None,
            agent_public_key_fingerprint: None,
        }
    }

    /// Constructs a boot-report submission with node-agent public identity.
    pub const fn new_with_agent_public_identity(
        report: HephaestusAgentBootReport,
        boot_secret: HephaestusAgentBootSecret,
        agent_public_key: HephaestusAgentPublicKey,
        agent_public_key_fingerprint: HephaestusAgentPublicKeyFingerprint,
    ) -> Self {
        Self {
            report,
            boot_secret,
            agent_public_key: Some(agent_public_key),
            agent_public_key_fingerprint: Some(agent_public_key_fingerprint),
        }
    }

    /// Returns the submitted boot report.
    pub const fn report(&self) -> &HephaestusAgentBootReport {
        &self.report
    }

    /// Returns the submitted boot secret.
    pub const fn boot_secret(&self) -> &HephaestusAgentBootSecret {
        &self.boot_secret
    }

    /// Returns the submitted node-agent public key, if this is first registration.
    pub const fn agent_public_key(&self) -> Option<&HephaestusAgentPublicKey> {
        self.agent_public_key.as_ref()
    }

    /// Returns the submitted public-key fingerprint, if this is first registration.
    pub const fn agent_public_key_fingerprint(
        &self,
    ) -> Option<&HephaestusAgentPublicKeyFingerprint> {
        self.agent_public_key_fingerprint.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterAgentRequest {
    report: HephaestusAgentBootReport,
    bootstrap_token: HephaestusAgentBootSecret,
    agent_version: String,
    agent_public_key: HephaestusAgentPublicKey,
    agent_public_key_fingerprint: HephaestusAgentPublicKeyFingerprint,
    agent_registration_signature: String,
}

impl RegisterAgentRequest {
    /// Constructs an agent first-boot registration request.
    pub fn new(
        report: HephaestusAgentBootReport,
        bootstrap_token: HephaestusAgentBootSecret,
        agent_version: String,
        agent_public_key: HephaestusAgentPublicKey,
        agent_public_key_fingerprint: HephaestusAgentPublicKeyFingerprint,
        agent_registration_signature: String,
    ) -> Self {
        Self {
            report,
            bootstrap_token,
            agent_version,
            agent_public_key,
            agent_public_key_fingerprint,
            agent_registration_signature,
        }
    }

    /// Returns the submitted first-boot report.
    pub const fn report(&self) -> &HephaestusAgentBootReport {
        &self.report
    }

    /// Returns the one-time bootstrap token.
    pub const fn bootstrap_token(&self) -> &HephaestusAgentBootSecret {
        &self.bootstrap_token
    }

    /// Returns the registering agent version.
    pub fn agent_version(&self) -> &str {
        self.agent_version.as_str()
    }

    /// Returns the node-generated public key.
    pub const fn agent_public_key(&self) -> &HephaestusAgentPublicKey {
        &self.agent_public_key
    }

    /// Returns the submitted public-key fingerprint.
    pub const fn agent_public_key_fingerprint(&self) -> &HephaestusAgentPublicKeyFingerprint {
        &self.agent_public_key_fingerprint
    }

    /// Returns the registration proof-of-possession signature.
    pub fn agent_registration_signature(&self) -> &str {
        self.agent_registration_signature.as_str()
    }
}

/// Builds the exact byte payload signed by agents during first-boot registration.
///
/// The payload intentionally excludes the bootstrap token. The signature proves
/// possession of the submitted private key, while the bootstrap token remains a
/// separate one-time authorization secret.
pub fn agent_registration_signature_payload(
    report: &HephaestusAgentBootReport,
    agent_version: &str,
    agent_public_key: &HephaestusAgentPublicKey,
    agent_public_key_fingerprint: &HephaestusAgentPublicKeyFingerprint,
) -> String {
    let mut payload = String::new();
    payload.push_str("reallyme:hephaestus-agent-register:v1\n");
    push_payload_field(&mut payload, "server_id", report.server_id().as_str());
    push_payload_field(
        &mut payload,
        "provider_server_id",
        report.provider_server_id().as_str(),
    );
    push_payload_field(&mut payload, "site_id", report.site_id().as_str());
    push_payload_field(
        &mut payload,
        "private_ip",
        report
            .private_ip()
            .map(|value| value.as_str())
            .unwrap_or(""),
    );
    push_payload_field(&mut payload, "agent_version", agent_version);
    push_payload_field(&mut payload, "agent_public_key", agent_public_key.as_str());
    push_payload_field(
        &mut payload,
        "agent_public_key_fingerprint",
        agent_public_key_fingerprint.as_str(),
    );
    payload
}

fn push_payload_field(payload: &mut String, name: &str, value: &str) {
    payload.push_str(name);
    payload.push('=');
    payload.push_str(value);
    payload.push('\n');
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterAgentResponse {
    result: HephaestusAgentBootReportResult,
    actual_state: Option<HephaestusNodeActualState>,
    runtime_token: Option<HephaestusAgentRuntimeToken>,
    desired_generation: u64,
    poll_after_seconds: u32,
}

impl RegisterAgentResponse {
    /// Constructs an agent registration response.
    pub const fn new(
        result: HephaestusAgentBootReportResult,
        actual_state: Option<HephaestusNodeActualState>,
        runtime_token: Option<HephaestusAgentRuntimeToken>,
        desired_generation: u64,
        poll_after_seconds: u32,
    ) -> Self {
        Self {
            result,
            actual_state,
            runtime_token,
            desired_generation,
            poll_after_seconds,
        }
    }

    /// Returns the stable registration result.
    pub const fn result(&self) -> HephaestusAgentBootReportResult {
        self.result
    }

    /// Returns the accepted node state, if registration was accepted.
    pub const fn actual_state(&self) -> Option<&HephaestusNodeActualState> {
        self.actual_state.as_ref()
    }

    /// Returns the runtime token issued to the node, if registration was accepted.
    pub const fn runtime_token(&self) -> Option<&HephaestusAgentRuntimeToken> {
        self.runtime_token.as_ref()
    }

    /// Returns the desired generation observed at registration time.
    pub const fn desired_generation(&self) -> u64 {
        self.desired_generation
    }

    /// Returns the server-advised polling cadence.
    pub const fn poll_after_seconds(&self) -> u32 {
        self.poll_after_seconds
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitAgentBootReportResponse {
    result: HephaestusAgentBootReportResult,
    actual_state: Option<HephaestusNodeActualState>,
}

impl SubmitAgentBootReportResponse {
    /// Constructs a boot-report submission response.
    pub const fn new(
        result: HephaestusAgentBootReportResult,
        actual_state: Option<HephaestusNodeActualState>,
    ) -> Self {
        Self {
            result,
            actual_state,
        }
    }

    /// Returns the stable boot-report result.
    pub const fn result(&self) -> HephaestusAgentBootReportResult {
        self.result
    }

    /// Returns the resulting actual node state when accepted.
    pub const fn actual_state(&self) -> Option<&HephaestusNodeActualState> {
        self.actual_state.as_ref()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListNodeActualStatesRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListNodeActualStatesResponse {
    nodes: Vec<HephaestusNodeActualState>,
}

impl ListNodeActualStatesResponse {
    /// Constructs a node-actual-state listing response.
    pub fn new(nodes: Vec<HephaestusNodeActualState>) -> Self {
        Self { nodes }
    }

    /// Returns the current actual node states.
    pub fn nodes(&self) -> &[HephaestusNodeActualState] {
        self.nodes.as_slice()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetNodeActualStateByServerIdRequest {
    server_id: HephaestusServerId,
}

impl GetNodeActualStateByServerIdRequest {
    /// Constructs an actual-state lookup request.
    pub const fn new(server_id: HephaestusServerId) -> Self {
        Self { server_id }
    }

    /// Returns the internal server identifier.
    pub const fn server_id(&self) -> &HephaestusServerId {
        &self.server_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetNodeActualStateByServerIdResponse {
    state: Option<HephaestusNodeActualState>,
}

impl GetNodeActualStateByServerIdResponse {
    /// Constructs an actual-state lookup response.
    pub const fn new(state: Option<HephaestusNodeActualState>) -> Self {
        Self { state }
    }

    /// Returns the actual node state when known.
    pub const fn state(&self) -> Option<&HephaestusNodeActualState> {
        self.state.as_ref()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListServerInventoryRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListServerInventoryResponse {
    rows: Vec<HephaestusServerInventoryRow>,
}

impl ListServerInventoryResponse {
    /// Constructs a server-inventory listing response.
    pub fn new(rows: Vec<HephaestusServerInventoryRow>) -> Self {
        Self { rows }
    }

    /// Returns the derived server inventory rows.
    pub fn rows(&self) -> &[HephaestusServerInventoryRow] {
        self.rows.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::agent_registration_signature_payload;
    use reallyme_hephaestus_domain::{
        HephaestusAgentBootReport, HephaestusAgentPublicKey, HephaestusAgentPublicKeyFingerprint,
        HephaestusPrivateIpAddress, HephaestusProviderServerId, HephaestusServerBootTiming,
        HephaestusServerId, HephaestusServerIdentity, HephaestusServerNetwork, HephaestusSiteId,
    };

    #[test]
    fn agent_registration_signature_payload_matches_expected_wire_format() {
        let report = HephaestusAgentBootReport::new(
            HephaestusServerIdentity::new(
                HephaestusServerId::new("srv-lhr-01").expect("valid server"),
                HephaestusProviderServerId::new("provider-server-1").expect("valid provider"),
                HephaestusSiteId::new("lhr").expect("valid site"),
            ),
            HephaestusServerNetwork::new(
                None,
                Some(HephaestusPrivateIpAddress::new("10.1.100.10").expect("valid private ip")),
            ),
            HephaestusServerBootTiming::new(1_700_000_000, 1_700_000_001).expect("valid timing"),
        )
        .expect("valid report");

        let fingerprint = "a".repeat(64);
        let payload = agent_registration_signature_payload(
            &report,
            "hephaestus-agent/1.2.3",
            &HephaestusAgentPublicKey::new("MDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDA")
                .expect("valid public key"),
            &HephaestusAgentPublicKeyFingerprint::new(fingerprint.as_str())
                .expect("valid fingerprint"),
        );

        let expected = format!(
            "reallyme:hephaestus-agent-register:v1\n\
server_id=srv-lhr-01\n\
provider_server_id=provider-server-1\n\
site_id=lhr\n\
private_ip=10.1.100.10\n\
agent_version=hephaestus-agent/1.2.3\n\
agent_public_key=MDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDA\n\
agent_public_key_fingerprint={fingerprint}\n"
        );

        assert_eq!(payload, expected);
    }
}
