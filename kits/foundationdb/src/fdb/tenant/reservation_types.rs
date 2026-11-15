// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

use super::tenant_id;
use crate::{FoundationDbConnector, FoundationDbTenantName};

/// Stable reservation failures for operator commands.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ReservationErrorReason {
    /// A reservation identity could not be generated.
    #[error("could not generate a reservation identity")]
    IdentityUnavailable,
    /// A configured control tenant is absent or has been replaced.
    #[error("reservation control tenant is missing or has a different ID")]
    ControlTenantChanged,
    /// The control tenant itself cannot be the target of a reserved delete.
    #[error("reservation control tenant cannot be a delete target")]
    ControlTenantTarget,
    /// A different operation holds the reservation for the target name.
    #[error("another operation holds the tenant reservation")]
    Busy,
    /// The current reservation is still within its takeover window.
    #[error("reservation is still active; retry takeover after expiry")]
    LeaseActive,
    /// There is no active reservation for a takeover by name.
    #[error("no active reservation for the tenant")]
    Missing,
    /// The operation has no completed cluster record to compact.
    #[error("reservation has no completed outcome to prune")]
    PruneNotCompleted,
    /// Reclamation requires a tombstone from a generation below the cutoff.
    #[error("reservation is not a tombstone below the admission cutoff")]
    ReclaimNotEligible,
    /// An intent predates the durable cutoff for its tenant name.
    #[error("delete intent expired at the tenant admission cutoff")]
    IntentExpired,
    /// An advance must use an explicit, consecutive nonzero target.
    #[error("cutoff target must name a nonzero generation at most one past the current cutoff")]
    InvalidCutoffTarget,
    /// The epoch or owner changed before the mutation could commit.
    #[error("reservation ownership changed; reconcile the recorded delete")]
    Stale,
    /// The delete cannot begin against a different ID.
    #[error("tenant mapping no longer has the recorded ID")]
    TargetChanged,
    /// A durable record failed structural or consistency validation.
    #[error("invalid reservation record")]
    InvalidRecord,
    /// Recovery or deletion could not validate the target contents.
    #[error("tenant contents prevent the reserved operation")]
    InvalidMetadata,
    /// Raw transactions must honor the tenant's administrative lock.
    #[error("reservation transaction requires an unlocked tenant")]
    TenantLocked,
    /// The bounded database operation failed or timed out.
    #[error("reservation transaction failed; reconcile the recorded delete")]
    TransactionFailed,
}

/// Result of an operator reservation operation.
pub type ReservationResult<T> = Result<T, ReservationErrorReason>;

/// Admission generation of a recorded intent for one tenant name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReservationGeneration(u64);

impl ReservationGeneration {
    /// Generation of existing records that predate the cutoff.
    pub const LEGACY: Self = Self(0);

    /// Constructs a generation from its durable integer representation.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the durable integer representation.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Random identifier for a durable delete intent.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ReservationId([u8; 16]);

impl ReservationId {
    /// Creates an ID before a local intent or takeover is submitted.
    pub fn generate() -> ReservationResult<Self> {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(|_| ReservationErrorReason::IdentityUnavailable)?;
        Ok(Self(bytes))
    }

    /// Constructs an ID decoded from an operator record.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Returns the fixed-width record representation.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Name and immutable ID of the dedicated control tenant.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ReservationControlTenant {
    name: FoundationDbTenantName,
    id: i64,
}

impl ReservationControlTenant {
    /// Validates a control tenant identity decoded from a durable record.
    pub fn new(name: FoundationDbTenantName, id: i64) -> ReservationResult<Self> {
        if id < 0 {
            return Err(ReservationErrorReason::InvalidRecord);
        }
        Ok(Self { name, id })
    }

    /// Returns the configured control tenant name.
    pub const fn name(self) -> FoundationDbTenantName {
        self.name
    }

    /// Returns the cluster-assigned ID of the control tenant.
    pub const fn id(self) -> i64 {
        self.id
    }

    /// Loads the control tenant ID from the current cluster.
    pub async fn lookup(
        connector: &FoundationDbConnector,
        name: FoundationDbTenantName,
    ) -> ReservationResult<Self> {
        let id = tenant_id(connector, name)
            .await
            .map_err(|_| ReservationErrorReason::TransactionFailed)?
            .ok_or(ReservationErrorReason::ControlTenantChanged)?;
        Self::new(name, id)
    }
}

/// Identity of one recorded deletion in a control tenant.
#[derive(Clone, Copy)]
pub struct ReservationIntent {
    /// Name that originally resolved to `original_id`.
    pub tenant: FoundationDbTenantName,
    /// Cluster-assigned ID that the delete and repair may affect.
    pub original_id: i64,
    /// Stable operation ID shared by the local and cluster records.
    pub operation: ReservationId,
    /// Generation captured with the original ID before the local file is persisted.
    pub generation: ReservationGeneration,
}
