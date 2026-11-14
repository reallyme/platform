// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use foundationdb::{Database, FdbError as DriverError, Transaction, options::TransactionOption};

use super::{TENANT_ADMIN_TIMEOUT, TENANT_ADMIN_TIMEOUT_MILLIS, TenantDeleteResolution, tenant_id};
use crate::{FoundationDbConnector, FoundationDbTenantName};

#[path = "reservation_types.rs"]
mod types;
pub use types::{
    ReservationControlTenant, ReservationErrorReason, ReservationId, ReservationIntent,
    ReservationResult,
};

#[path = "reservation_record.rs"]
mod record;
use record::{Phase, StoredReservation};

#[path = "reservation_transactions.rs"]
mod transactions;
use transactions::{mapping_matches, stage_clear, stage_delete, stage_repair};

const NAMESPACE: &[u8] = b"__meta/reallyme/delete-reservation/v1/";
const RESERVATION_RETRIES: i32 = 2;
// Expiry controls availability. Transfer of the epoch, rather than a local
// clock reading, revokes the old worker's authority at the database.
const LEASE_VERSIONS: i64 = 15_000_000;

/// Access to the reservation records of one dedicated control tenant.
pub struct ReservationStore<'a> {
    database: &'a Database,
    control: ReservationControlTenant,
}

#[derive(Clone, Copy)]
struct Grant {
    intent: ReservationIntent,
    claimant: ReservationId,
    epoch: u64,
    phase: Phase,
}

enum Claim {
    Acquired(Grant),
    Complete(TenantDeleteResolution),
}

enum AttemptError {
    Driver(DriverError),
    Rejected(ReservationErrorReason),
}

impl From<DriverError> for AttemptError {
    fn from(error: DriverError) -> Self {
        Self::Driver(error)
    }
}

impl From<ReservationErrorReason> for AttemptError {
    fn from(reason: ReservationErrorReason) -> Self {
        Self::Rejected(reason)
    }
}

type AttemptResult<T> = Result<T, AttemptError>;

impl<'a> ReservationStore<'a> {
    /// Binds the store to a control tenant. Each transaction verifies its ID.
    /// A caller loading a record must also verify that it belongs to the
    /// connected cluster before constructing the store.
    pub fn new(connector: &'a FoundationDbConnector, control: ReservationControlTenant) -> Self {
        Self {
            database: connector.database(),
            control,
        }
    }

    /// Acquires an intent and deletes only its recorded tenant. A failed or
    /// interrupted operation remains available for coordinated reconciliation.
    pub async fn delete(
        &self,
        intent: ReservationIntent,
    ) -> ReservationResult<TenantDeleteResolution> {
        let grant = self.acquire(intent).await?;
        self.transact(async |transaction| {
            self.check_grant(transaction, grant).await?;
            if mapping_matches(transaction, intent.tenant, intent.original_id).await? {
                stage_clear(transaction, intent.tenant, intent.original_id).await?;
            }
            Ok(())
        })
        .await?;
        self.transact(async |transaction| {
            self.check_grant(transaction, grant).await?;
            stage_delete(transaction, intent).await
        })
        .await?;
        self.finish(grant).await
    }

    /// Returns a completed outcome or takes over an expired recorded intent.
    /// After takeover, the command reconciles and never starts a new delete.
    pub async fn resume(
        &self,
        intent: ReservationIntent,
    ) -> ReservationResult<TenantDeleteResolution> {
        match self.claim_recovery(intent).await? {
            Claim::Complete(outcome) => Ok(outcome),
            Claim::Acquired(grant) => self.finish(grant).await,
        }
    }

    /// Reconciles the active reservation after expiry without a local file.
    pub async fn takeover(
        &self,
        tenant: FoundationDbTenantName,
    ) -> ReservationResult<TenantDeleteResolution> {
        self.validate_target(tenant)?;
        let intent = self
            .transact(async |transaction| {
                let operation = self
                    .active_id(transaction, tenant)
                    .await?
                    .ok_or(ReservationErrorReason::Missing)?;
                let stored = self
                    .read_record(transaction, tenant, operation)
                    .await?
                    .ok_or(ReservationErrorReason::InvalidRecord)?;
                Ok(ReservationIntent {
                    tenant,
                    original_id: stored.original_id,
                    operation,
                })
            })
            .await?;
        self.resume(intent).await
    }

    async fn acquire(&self, intent: ReservationIntent) -> ReservationResult<Grant> {
        self.validate_intent(intent)?;
        self.transact(async |transaction| {
            let active = self.active_id(transaction, intent.tenant).await?;
            if let Some(stored) = self
                .read_record(transaction, intent.tenant, intent.operation)
                .await?
            {
                // Replay after an ambiguous acquisition may observe just this
                // grant. It cannot reset a transferred or completed record.
                if active == Some(intent.operation)
                    && stored.original_id == intent.original_id
                    && stored.phase == Phase::Deleting
                    && stored.claimant == intent.operation
                {
                    return Ok(Grant {
                        intent,
                        claimant: stored.claimant,
                        epoch: stored.epoch,
                        phase: stored.phase,
                    });
                }
                return Err(ReservationErrorReason::Stale.into());
            }
            if active.is_some() {
                return Err(ReservationErrorReason::Busy.into());
            }
            if !mapping_matches(transaction, intent.tenant, intent.original_id).await? {
                return Err(ReservationErrorReason::TargetChanged.into());
            }
            let grant = Grant {
                intent,
                claimant: intent.operation,
                epoch: 1,
                phase: Phase::Deleting,
            };
            self.write_grant(transaction, grant).await?;
            Ok(grant)
        })
        .await
    }

    async fn claim_recovery(&self, intent: ReservationIntent) -> ReservationResult<Claim> {
        self.validate_intent(intent)?;
        let claimant = ReservationId::generate()?;
        self.transact(async |transaction| {
            let active = self.active_id(transaction, intent.tenant).await?;
            let epoch = match self
                .read_record(transaction, intent.tenant, intent.operation)
                .await?
            {
                Some(stored) => {
                    if stored.original_id != intent.original_id {
                        return Err(ReservationErrorReason::InvalidRecord.into());
                    }
                    if let Phase::Complete(outcome) = stored.phase {
                        if active == Some(intent.operation) {
                            return Err(ReservationErrorReason::InvalidRecord.into());
                        }
                        return Ok(Claim::Complete(outcome));
                    }
                    if active != Some(intent.operation) {
                        return Err(ReservationErrorReason::InvalidRecord.into());
                    }
                    if stored.phase == Phase::Recovering && stored.claimant == claimant {
                        return Ok(Claim::Acquired(Grant {
                            intent,
                            claimant,
                            epoch: stored.epoch,
                            phase: stored.phase,
                        }));
                    }
                    if transaction.get_read_version().await? < stored.expires_at {
                        return Err(ReservationErrorReason::LeaseActive.into());
                    }
                    stored
                        .epoch
                        .checked_add(1)
                        .ok_or(ReservationErrorReason::InvalidRecord)?
                }
                None if active == Some(intent.operation) => {
                    return Err(ReservationErrorReason::InvalidRecord.into());
                }
                None if active.is_some() => return Err(ReservationErrorReason::Busy.into()),
                // The local intent can survive a crash before acquisition.
                // Publish recovery-only ownership before touching metadata.
                None => 1,
            };
            let grant = Grant {
                intent,
                claimant,
                epoch,
                phase: Phase::Recovering,
            };
            self.write_grant(transaction, grant).await?;
            Ok(Claim::Acquired(grant))
        })
        .await
    }

    async fn finish(&self, grant: Grant) -> ReservationResult<TenantDeleteResolution> {
        self.transact(async |transaction| self.stage_finish(transaction, grant).await)
            .await
    }

    async fn stage_finish(
        &self,
        transaction: &Transaction,
        grant: Grant,
    ) -> AttemptResult<TenantDeleteResolution> {
        let intent = grant.intent;
        let mut stored = self
            .read_record(transaction, intent.tenant, intent.operation)
            .await?
            .ok_or(ReservationErrorReason::Stale)?;
        Self::check_epoch(stored, grant)?;
        if let Phase::Complete(outcome) = stored.phase {
            return Ok(outcome);
        }
        self.check_active(transaction, stored, grant).await?;
        let outcome = if !mapping_matches(transaction, intent.tenant, intent.original_id).await? {
            TenantDeleteResolution::OriginalAbsent
        } else if grant.phase == Phase::Recovering {
            stage_repair(transaction, intent.tenant, intent.original_id).await?;
            TenantDeleteResolution::MetadataSafe
        } else {
            return Err(ReservationErrorReason::TargetChanged.into());
        };
        stored.phase = Phase::Complete(outcome);
        transaction.set(
            &self.record_key(intent.tenant, intent.operation),
            &stored.encode(),
        );
        transaction.clear(&self.active_key(intent.tenant));
        Ok(outcome)
    }

    fn validate_target(&self, tenant: FoundationDbTenantName) -> ReservationResult<()> {
        if tenant == self.control.name() {
            return Err(ReservationErrorReason::ControlTenantTarget);
        }
        Ok(())
    }

    fn validate_intent(&self, intent: ReservationIntent) -> ReservationResult<()> {
        self.validate_target(intent.tenant)?;
        if intent.original_id == self.control.id() {
            return Err(ReservationErrorReason::ControlTenantTarget);
        }
        if intent.original_id < 0 {
            return Err(ReservationErrorReason::InvalidRecord);
        }
        Ok(())
    }

    async fn check_grant(&self, transaction: &Transaction, grant: Grant) -> AttemptResult<()> {
        let intent = grant.intent;
        let stored = self
            .read_record(transaction, intent.tenant, intent.operation)
            .await?
            .ok_or(ReservationErrorReason::Stale)?;
        Self::check_epoch(stored, grant)?;
        self.check_active(transaction, stored, grant).await
    }

    fn check_epoch(stored: StoredReservation, grant: Grant) -> ReservationResult<()> {
        if stored.original_id != grant.intent.original_id
            || stored.epoch != grant.epoch
            || stored.claimant != grant.claimant
        {
            return Err(ReservationErrorReason::Stale);
        }
        Ok(())
    }

    async fn check_active(
        &self,
        transaction: &Transaction,
        stored: StoredReservation,
        grant: Grant,
    ) -> AttemptResult<()> {
        if stored.phase != grant.phase
            || self.active_id(transaction, grant.intent.tenant).await?
                != Some(grant.intent.operation)
        {
            return Err(ReservationErrorReason::Stale.into());
        }
        Ok(())
    }

    async fn transact<T>(
        &self,
        operation: impl AsyncFn(&Transaction) -> AttemptResult<T>,
    ) -> ReservationResult<T> {
        let failed = ReservationErrorReason::TransactionFailed;
        tokio::time::timeout(TENANT_ADMIN_TIMEOUT, async {
            let mut transaction = self.database.create_trx().map_err(|_| failed)?;
            loop {
                configure_transaction(&transaction).map_err(|_| failed)?;
                // Repeat the control-ID and epoch checks after every reset.
                let result =
                    match mapping_matches(&transaction, self.control.name(), self.control.id())
                        .await
                    {
                        Ok(true) => operation(&transaction).await,
                        Ok(false) => return Err(ReservationErrorReason::ControlTenantChanged),
                        Err(error) => Err(error),
                    };
                transaction = match result {
                    Ok(value) => match transaction.commit().await {
                        Ok(_) => return Ok(value),
                        Err(error) => error.on_error().await.map_err(|_| failed)?,
                    },
                    Err(AttemptError::Driver(error)) => {
                        transaction.on_error(error).await.map_err(|_| failed)?
                    }
                    Err(AttemptError::Rejected(reason)) => return Err(reason),
                };
            }
        })
        .await
        .map_err(|_| failed)?
    }

    fn active_key(&self, tenant: FoundationDbTenantName) -> Vec<u8> {
        [
            self.control.id().to_be_bytes().as_slice(),
            NAMESPACE,
            b"active/",
            tenant.as_bytes(),
        ]
        .concat()
    }

    fn record_key(&self, tenant: FoundationDbTenantName, operation: ReservationId) -> Vec<u8> {
        [
            self.control.id().to_be_bytes().as_slice(),
            NAMESPACE,
            b"record/",
            tenant.as_bytes(),
            b"/",
            operation.as_bytes(),
        ]
        .concat()
    }

    async fn active_id(
        &self,
        transaction: &Transaction,
        tenant: FoundationDbTenantName,
    ) -> AttemptResult<Option<ReservationId>> {
        transaction
            .get(&self.active_key(tenant), false)
            .await?
            .map(|value| {
                let bytes = value
                    .as_ref()
                    .try_into()
                    .map_err(|_| ReservationErrorReason::InvalidRecord)?;
                Ok(ReservationId::from_bytes(bytes))
            })
            .transpose()
    }

    async fn read_record(
        &self,
        transaction: &Transaction,
        tenant: FoundationDbTenantName,
        operation: ReservationId,
    ) -> AttemptResult<Option<StoredReservation>> {
        Ok(transaction
            .get(&self.record_key(tenant, operation), false)
            .await?
            .map(|value| StoredReservation::decode(value.as_ref()))
            .transpose()?)
    }

    async fn write_grant(&self, transaction: &Transaction, grant: Grant) -> AttemptResult<()> {
        let expires_at = transaction
            .get_read_version()
            .await?
            .checked_add(LEASE_VERSIONS)
            .ok_or(ReservationErrorReason::InvalidRecord)?;
        let intent = grant.intent;
        let record = StoredReservation {
            original_id: intent.original_id,
            epoch: grant.epoch,
            claimant: grant.claimant,
            expires_at,
            phase: grant.phase,
        };
        transaction.set(
            &self.record_key(intent.tenant, intent.operation),
            &record.encode(),
        );
        transaction.set(&self.active_key(intent.tenant), intent.operation.as_bytes());
        Ok(())
    }
}

fn configure_transaction(transaction: &Transaction) -> Result<(), DriverError> {
    transaction.set_option(TransactionOption::RawAccess)?;
    transaction.set_option(TransactionOption::ReadSystemKeys)?;
    transaction.set_option(TransactionOption::Timeout(TENANT_ADMIN_TIMEOUT_MILLIS))?;
    transaction.set_option(TransactionOption::RetryLimit(RESERVATION_RETRIES))?;
    Ok(())
}

#[cfg(test)]
#[path = "reservation_recovery_tests.rs"]
mod tests;
