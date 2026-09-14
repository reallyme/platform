// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Generic host-neutral app port error.
///
/// Concrete app ports may define richer typed errors when needed. This shared
/// error covers the common deterministic/fail-closed downstream boundary cases
/// without pulling host, transport, or network client types into app-kit.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AppPortError {
    /// The port was intentionally left unconfigured.
    #[error("app port is unconfigured")]
    Unconfigured,
    /// The port operation timed out before completion.
    #[error("app port operation timed out")]
    Timeout,
    /// The remote downstream port was unavailable.
    #[error("app downstream port unavailable")]
    Unavailable,
    /// The downstream response violated the expected contract.
    #[error("app port protocol violation")]
    ProtocolViolation,
    /// The requested downstream operation is not implemented.
    #[error("app port operation not implemented")]
    NotImplemented,
    /// Hetzner create failed before a provider API mutation was attempted.
    #[error("hetzner create preflight failed")]
    HetznerCreatePreflight,
    /// Hetzner create could not acquire the environment execution lock.
    #[error("hetzner create environment lock failed")]
    HetznerCreateEnvironmentLock,
    /// Hetzner create could not reconcile existing provider state.
    #[error("hetzner create provider reconciliation failed")]
    HetznerCreateReconcileMissing,
    /// Hetzner create could not allocate durable desired-node state.
    #[error("hetzner create node allocation failed")]
    HetznerCreateAllocateNodes,
    /// Hetzner create failed while calling the provider API.
    #[error("hetzner create provider API failed")]
    HetznerCreateProviderApi,
    /// Hetzner create succeeded at the provider but failed to persist local state.
    #[error("hetzner create local state update failed")]
    HetznerCreateUpsertNode,
    /// Hetzner delete could not acquire the environment execution lock.
    #[error("hetzner delete environment lock failed")]
    HetznerDeleteEnvironmentLock,
    /// Hetzner delete failed while calling the provider API.
    #[error("hetzner delete provider API failed")]
    HetznerDeleteProviderApi,
    /// Hetzner delete succeeded at the provider but failed to persist local state.
    #[error("hetzner delete local state update failed")]
    HetznerDeleteUpdateState,
    /// Hetzner delete succeeded at the provider but tailnet cleanup failed.
    #[error("hetzner delete tailnet cleanup failed")]
    HetznerDeleteTailnetCleanup,
    /// Hetzner power action could not acquire the environment execution lock.
    #[error("hetzner power action environment lock failed")]
    HetznerPowerEnvironmentLock,
    /// Hetzner power action failed while calling the provider API.
    #[error("hetzner power action provider API failed")]
    HetznerPowerProviderApi,
    /// Hetzner resize could not acquire the environment execution lock.
    #[error("hetzner resize environment lock failed")]
    HetznerResizeEnvironmentLock,
    /// Hetzner resize failed while calling the provider API.
    #[error("hetzner resize provider API failed")]
    HetznerResizeProviderApi,
    /// Hetzner resize succeeded at the provider but failed to persist local state.
    #[error("hetzner resize local state update failed")]
    HetznerResizeUpdateState,
    /// Vultr create failed before a provider API mutation was attempted.
    #[error("vultr create preflight failed")]
    VultrCreatePreflight,
    /// Vultr create could not acquire the environment execution lock.
    #[error("vultr create environment lock failed")]
    VultrCreateEnvironmentLock,
    /// Vultr create could not reconcile existing provider state.
    #[error("vultr create provider reconciliation failed")]
    VultrCreateReconcileMissing,
    /// Vultr create could not allocate durable desired-node state.
    #[error("vultr create node allocation failed")]
    VultrCreateAllocateNodes,
    /// Vultr create failed while calling the provider API.
    #[error("vultr create provider API failed")]
    VultrCreateProviderApi,
    /// Vultr create succeeded at the provider but failed to persist local state.
    #[error("vultr create local state update failed")]
    VultrCreateUpsertNode,
    /// Vultr delete could not acquire the environment execution lock.
    #[error("vultr delete environment lock failed")]
    VultrDeleteEnvironmentLock,
    /// Vultr delete failed while calling the provider API.
    #[error("vultr delete provider API failed")]
    VultrDeleteProviderApi,
    /// Vultr delete succeeded at the provider but failed to persist local state.
    #[error("vultr delete local state update failed")]
    VultrDeleteUpdateState,
    /// Vultr delete succeeded at the provider but tailnet cleanup failed.
    #[error("vultr delete tailnet cleanup failed")]
    VultrDeleteTailnetCleanup,
    /// Vultr power action could not acquire the environment execution lock.
    #[error("vultr power action environment lock failed")]
    VultrPowerEnvironmentLock,
    /// Vultr power action failed while calling the provider API.
    #[error("vultr power action provider API failed")]
    VultrPowerProviderApi,
    /// Vultr resize could not acquire the environment execution lock.
    #[error("vultr resize environment lock failed")]
    VultrResizeEnvironmentLock,
    /// Vultr resize failed while calling the provider API.
    #[error("vultr resize provider API failed")]
    VultrResizeProviderApi,
    /// Vultr resize succeeded at the provider but failed to persist local state.
    #[error("vultr resize local state update failed")]
    VultrResizeUpdateState,
}

impl AppPortError {
    /// Returns the low-cardinality metric label for this port error.
    pub const fn as_metric_label(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::Timeout => "timeout",
            Self::Unavailable => "unavailable",
            Self::ProtocolViolation => "protocol_violation",
            Self::NotImplemented => "not_implemented",
            Self::HetznerCreatePreflight => "hetzner_create_preflight",
            Self::HetznerCreateEnvironmentLock => "hetzner_create_environment_lock",
            Self::HetznerCreateReconcileMissing => "hetzner_create_reconcile_missing",
            Self::HetznerCreateAllocateNodes => "hetzner_create_allocate_nodes",
            Self::HetznerCreateProviderApi => "hetzner_create_provider_api",
            Self::HetznerCreateUpsertNode => "hetzner_create_upsert_node",
            Self::HetznerDeleteEnvironmentLock => "hetzner_delete_environment_lock",
            Self::HetznerDeleteProviderApi => "hetzner_delete_provider_api",
            Self::HetznerDeleteUpdateState => "hetzner_delete_update_state",
            Self::HetznerDeleteTailnetCleanup => "hetzner_delete_tailnet_cleanup",
            Self::HetznerPowerEnvironmentLock => "hetzner_power_environment_lock",
            Self::HetznerPowerProviderApi => "hetzner_power_provider_api",
            Self::HetznerResizeEnvironmentLock => "hetzner_resize_environment_lock",
            Self::HetznerResizeProviderApi => "hetzner_resize_provider_api",
            Self::HetznerResizeUpdateState => "hetzner_resize_update_state",
            Self::VultrCreatePreflight => "vultr_create_preflight",
            Self::VultrCreateEnvironmentLock => "vultr_create_environment_lock",
            Self::VultrCreateReconcileMissing => "vultr_create_reconcile_missing",
            Self::VultrCreateAllocateNodes => "vultr_create_allocate_nodes",
            Self::VultrCreateProviderApi => "vultr_create_provider_api",
            Self::VultrCreateUpsertNode => "vultr_create_upsert_node",
            Self::VultrDeleteEnvironmentLock => "vultr_delete_environment_lock",
            Self::VultrDeleteProviderApi => "vultr_delete_provider_api",
            Self::VultrDeleteUpdateState => "vultr_delete_update_state",
            Self::VultrDeleteTailnetCleanup => "vultr_delete_tailnet_cleanup",
            Self::VultrPowerEnvironmentLock => "vultr_power_environment_lock",
            Self::VultrPowerProviderApi => "vultr_power_provider_api",
            Self::VultrResizeEnvironmentLock => "vultr_resize_environment_lock",
            Self::VultrResizeProviderApi => "vultr_resize_provider_api",
            Self::VultrResizeUpdateState => "vultr_resize_update_state",
        }
    }
}

/// Backward-compatible alias for call sites that were binding a separate error-kind
/// type. Prefer [`AppPortError`] with [`AppPortError::as_metric_label()`].
pub type AppPortErrorKind = AppPortError;

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
