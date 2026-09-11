// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

use crate::{
    HephaestusAgentReport, HephaestusDnsRecord, HephaestusDomainError,
    HephaestusInfrastructureClassId, HephaestusNodeActualState, HephaestusNodeActualStateStatus,
    HephaestusOrchestratorNodeState, HephaestusPrivateIpAddress, HephaestusPublicIpAddress,
    HephaestusRegionId, HephaestusServerCatalog, HephaestusServerId, HephaestusServerIdentity,
    ProvisioningEventSnapshot,
};

/// Broad environment safety tag for server management UI and action review.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusServerEnvironmentTag {
    /// Environment is not known yet.
    #[default]
    Unspecified,
    /// Development-like, lab, smoke, staging, or otherwise non-production.
    Dev,
    /// Production server.
    Prod,
}

/// Derived dashboard inventory row for one Hephaestus-managed server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusServerInventoryRow {
    identity: HephaestusServerIdentity,
    assigned_region_id: Option<HephaestusRegionId>,
    environment_tag: HephaestusServerEnvironmentTag,
    catalog: HephaestusServerCatalog,
    infrastructure_class_id: HephaestusInfrastructureClassId,
    actual_apps: Vec<crate::HephaestusServerApp>,
    planned_private_ip: Option<HephaestusPrivateIpAddress>,
    public_ip: Option<HephaestusPublicIpAddress>,
    private_ip: Option<HephaestusPrivateIpAddress>,
    boot_time_unix_secs: Option<u64>,
    last_seen_unix_secs: Option<u64>,
    actual_status: Option<HephaestusNodeActualStateStatus>,
    latest_provisioning_event: Option<ProvisioningEventSnapshot>,
    dns_records: Vec<HephaestusDnsRecord>,
    latest_agent_report: Option<HephaestusAgentReport>,
}

impl HephaestusServerInventoryRow {
    /// Constructs a validated inventory row from joined control-plane state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: HephaestusServerIdentity,
        assigned_region_id: Option<HephaestusRegionId>,
        catalog: HephaestusServerCatalog,
        infrastructure_class_id: HephaestusInfrastructureClassId,
        actual_apps: Vec<crate::HephaestusServerApp>,
        planned_private_ip: Option<HephaestusPrivateIpAddress>,
        public_ip: Option<HephaestusPublicIpAddress>,
        private_ip: Option<HephaestusPrivateIpAddress>,
        boot_time_unix_secs: Option<u64>,
        last_seen_unix_secs: Option<u64>,
        actual_status: Option<HephaestusNodeActualStateStatus>,
        latest_provisioning_event: Option<ProvisioningEventSnapshot>,
        dns_records: Vec<HephaestusDnsRecord>,
        latest_agent_report: Option<HephaestusAgentReport>,
    ) -> Result<Self, HephaestusDomainError> {
        match (boot_time_unix_secs, last_seen_unix_secs) {
            (Some(boot_time), Some(last_seen))
                if boot_time == 0 || last_seen == 0 || boot_time > last_seen =>
            {
                return Err(HephaestusDomainError::InvalidNumber);
            }
            (Some(0), _) | (_, Some(0)) => return Err(HephaestusDomainError::InvalidNumber),
            _ => {}
        }

        Ok(Self {
            identity,
            assigned_region_id,
            environment_tag: HephaestusServerEnvironmentTag::Unspecified,
            catalog,
            infrastructure_class_id,
            actual_apps,
            planned_private_ip,
            public_ip,
            private_ip,
            boot_time_unix_secs,
            last_seen_unix_secs,
            actual_status,
            latest_provisioning_event,
            dns_records,
            latest_agent_report,
        })
    }

    /// Constructs an inventory row from expected orchestrator state.
    pub fn from_orchestrator_state(
        value: &HephaestusOrchestratorNodeState,
    ) -> Result<Self, HephaestusDomainError> {
        Self::new(
            value.identity().clone(),
            None,
            value.catalog().clone(),
            value.infrastructure_class_id().clone(),
            Vec::new(),
            value.expected_private_ip().cloned(),
            None,
            None,
            None,
            None,
            None,
            None,
            Vec::new(),
            None,
        )
    }

    /// Overlays actual-state data onto the inventory row.
    pub fn with_actual_state(
        mut self,
        value: &HephaestusNodeActualState,
    ) -> Result<Self, HephaestusDomainError> {
        self.identity = value.identity().clone();
        self.catalog = value.catalog().clone();
        self.infrastructure_class_id = value.infrastructure_class_id().clone();
        self.public_ip = value.public_ip().cloned();
        self.private_ip = value.private_ip().cloned();
        self.boot_time_unix_secs = Some(value.boot_time_unix_secs());
        self.last_seen_unix_secs = Some(value.last_seen_unix_secs());
        self.actual_status = Some(value.status());
        Ok(self)
    }

    /// Overlays node-local liveness onto an existing desired/provider row.
    ///
    /// Provisioning and provider inventory remain the authority for durable
    /// identity, planned role, public address, and instance class. Agent actual
    /// state is the authority for liveness timestamps and the node-local
    /// private address that should be used for operational health.
    pub fn with_actual_observation(
        mut self,
        value: &HephaestusNodeActualState,
    ) -> Result<Self, HephaestusDomainError> {
        self.private_ip = value.private_ip().cloned().or(self.private_ip);
        self.public_ip = value.public_ip().cloned().or(self.public_ip);
        self.boot_time_unix_secs = Some(value.boot_time_unix_secs());
        self.last_seen_unix_secs = Some(value.last_seen_unix_secs());
        self.actual_status = Some(value.status());
        Ok(self)
    }

    /// Overlays the latest provisioning event onto the inventory row.
    pub fn with_latest_provisioning_event(mut self, value: ProvisioningEventSnapshot) -> Self {
        self.latest_provisioning_event = Some(value);
        self
    }

    /// Overlays the latest agent report onto the inventory row.
    pub fn with_latest_agent_report(mut self, value: HephaestusAgentReport) -> Self {
        self.actual_apps = value.actual_apps().to_vec();
        self.latest_agent_report = Some(value);
        self
    }

    /// Replaces DNS records associated with this server.
    pub fn with_dns_records(mut self, value: Vec<HephaestusDnsRecord>) -> Self {
        self.dns_records = value;
        self
    }

    /// Returns the joined identity.
    pub const fn identity(&self) -> &HephaestusServerIdentity {
        &self.identity
    }

    /// Returns the assigned operational Region when known.
    pub const fn assigned_region_id(&self) -> Option<&HephaestusRegionId> {
        self.assigned_region_id.as_ref()
    }

    /// Returns the broad environment safety tag.
    pub const fn environment_tag(&self) -> HephaestusServerEnvironmentTag {
        self.environment_tag
    }

    /// Returns the joined catalog.
    pub const fn catalog(&self) -> &HephaestusServerCatalog {
        &self.catalog
    }

    /// Returns the internal server id.
    pub const fn server_id(&self) -> &HephaestusServerId {
        self.identity.server_id()
    }

    /// Returns the provider-backed server identifier.
    pub const fn provider_server_id(&self) -> &crate::HephaestusProviderServerId {
        self.identity.provider_server_id()
    }

    /// Returns the infrastructure class identifier recorded for this server.
    pub const fn infrastructure_class_id(&self) -> &HephaestusInfrastructureClassId {
        &self.infrastructure_class_id
    }

    /// Returns actual ReallyMe apps discovered on this server.
    pub fn actual_apps(&self) -> &[crate::HephaestusServerApp] {
        self.actual_apps.as_slice()
    }

    /// Returns the planned private IP when known.
    pub const fn planned_private_ip(&self) -> Option<&HephaestusPrivateIpAddress> {
        self.planned_private_ip.as_ref()
    }

    /// Returns the observed public IP when known.
    pub const fn public_ip(&self) -> Option<&HephaestusPublicIpAddress> {
        self.public_ip.as_ref()
    }

    /// Returns the observed private IP when known.
    pub const fn private_ip(&self) -> Option<&HephaestusPrivateIpAddress> {
        self.private_ip.as_ref()
    }

    /// Returns the observed boot time when known.
    pub const fn boot_time_unix_secs(&self) -> Option<u64> {
        self.boot_time_unix_secs
    }

    /// Returns the last-seen timestamp when known.
    pub const fn last_seen_unix_secs(&self) -> Option<u64> {
        self.last_seen_unix_secs
    }

    /// Returns the actual readiness status when known.
    pub const fn actual_status(&self) -> Option<HephaestusNodeActualStateStatus> {
        self.actual_status
    }

    /// Returns the latest provisioning event when known.
    pub const fn latest_provisioning_event(&self) -> Option<&ProvisioningEventSnapshot> {
        self.latest_provisioning_event.as_ref()
    }

    /// Returns DNS records associated with this server.
    pub fn dns_records(&self) -> &[HephaestusDnsRecord] {
        self.dns_records.as_slice()
    }

    /// Returns the latest agent report when known.
    pub const fn latest_agent_report(&self) -> Option<&HephaestusAgentReport> {
        self.latest_agent_report.as_ref()
    }

    /// Returns a copy with an assigned operational Region.
    pub fn with_assigned_region_id(mut self, value: Option<HephaestusRegionId>) -> Self {
        self.assigned_region_id = value;
        self
    }

    /// Returns a copy with a broad environment safety tag.
    pub const fn with_environment_tag(mut self, value: HephaestusServerEnvironmentTag) -> Self {
        self.environment_tag = value;
        self
    }
}
