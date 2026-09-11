// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Planned node-agent capabilities for Hephaestus-managed servers.

use serde::Serialize;

/// Rollout phase for a Hephaestus agent capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusAgentRolloutPhase {
    /// Capability intentionally ships after the initial control-plane surface.
    AfterCoreControlPlane,
}

/// One planned Hephaestus node-agent capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HephaestusAgentCapability {
    /// Stable capability identifier.
    pub id: &'static str,
    /// Operator-facing capability label.
    pub label: &'static str,
    /// Rollout phase for this capability.
    pub phase: HephaestusAgentRolloutPhase,
}

/// Static catalog of planned agent capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HephaestusAgentCapabilityCatalog {
    /// Planned capabilities.
    pub capabilities: &'static [HephaestusAgentCapability],
}

/// Returns the planned Hephaestus agent capability catalog.
///
/// The agent is deliberately not a desired-state authority. The control plane
/// owns topology, workflows, and health coordination. The node agent exposes
/// host-local operations and telemetry that cannot be derived safely from
/// topology documents or workflow output alone.
pub const fn agent_capability_catalog() -> HephaestusAgentCapabilityCatalog {
    HephaestusAgentCapabilityCatalog {
        capabilities: &[
            HephaestusAgentCapability {
                id: "container_status",
                label: "container status",
                phase: HephaestusAgentRolloutPhase::AfterCoreControlPlane,
            },
            HephaestusAgentCapability {
                id: "logs",
                label: "logs",
                phase: HephaestusAgentRolloutPhase::AfterCoreControlPlane,
            },
            HephaestusAgentCapability {
                id: "host_resources",
                label: "CPU, memory, and disk",
                phase: HephaestusAgentRolloutPhase::AfterCoreControlPlane,
            },
            HephaestusAgentCapability {
                id: "restart_app",
                label: "restart app",
                phase: HephaestusAgentRolloutPhase::AfterCoreControlPlane,
            },
            HephaestusAgentCapability {
                id: "pull_new_image",
                label: "pull new GitHub container image",
                phase: HephaestusAgentRolloutPhase::AfterCoreControlPlane,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::{HephaestusAgentRolloutPhase, agent_capability_catalog};

    #[test]
    fn every_agent_capability_is_after_core_control_plane() {
        assert!(agent_capability_catalog().capabilities.iter().all(
            |capability| capability.phase == HephaestusAgentRolloutPhase::AfterCoreControlPlane
        ));
    }
}
