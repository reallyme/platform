// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use foundationdb::Transaction;

use super::{
    AttemptResult, Claim, Grant, Phase, ReservationErrorReason, ReservationId, ReservationIntent,
    ReservationRecord, ReservationResult, ReservationStore, mapping_matches,
};

impl ReservationStore<'_> {
    pub(super) async fn acquire(&self, intent: ReservationIntent) -> ReservationResult<Grant> {
        self.validate_intent(intent)?;
        self.transact(async |transaction| self.stage_acquire(transaction, intent).await)
            .await
    }

    pub(super) async fn stage_acquire(
        &self,
        transaction: &Transaction,
        intent: ReservationIntent,
    ) -> AttemptResult<Grant> {
        self.check_cutoff(transaction, intent).await?;
        let active = self.active_id(transaction, intent.tenant).await?;
        if let Some(record) = self
            .read_record(transaction, intent.tenant, intent.operation)
            .await?
        {
            // An ambiguous acquisition can replay this grant while it is
            // current. A transferred or completed record holds the slot closed.
            if let ReservationRecord::Full(stored) = record
                && active == Some(intent.operation)
                && stored.original_id == intent.original_id
                && stored.generation == intent.generation
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
    }

    pub(super) async fn claim_recovery(
        &self,
        intent: ReservationIntent,
    ) -> ReservationResult<Claim> {
        self.validate_intent(intent)?;
        let claimant = ReservationId::generate()?;
        self.transact(async |transaction| {
            self.stage_claim_recovery(transaction, intent, claimant)
                .await
        })
        .await
    }

    pub(super) async fn stage_claim_recovery(
        &self,
        transaction: &Transaction,
        intent: ReservationIntent,
        claimant: ReservationId,
    ) -> AttemptResult<Claim> {
        let active = self.active_id(transaction, intent.tenant).await?;
        let epoch = match self
            .read_record(transaction, intent.tenant, intent.operation)
            .await?
        {
            Some(ReservationRecord::Tombstone(completion)) => {
                return Self::completed_outcome(active, intent, completion).map(Claim::Complete);
            }
            Some(ReservationRecord::Full(stored)) => {
                if stored.original_id != intent.original_id
                    || stored.generation != intent.generation
                {
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
                self.check_cutoff(transaction, intent).await?;
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
            None => {
                if active == Some(intent.operation) {
                    return Err(ReservationErrorReason::InvalidRecord.into());
                }
                // A crash can leave a file without a cluster row. It may
                // start recovery only in the file's original generation.
                self.check_cutoff(transaction, intent).await?;
                if active.is_some() {
                    return Err(ReservationErrorReason::Busy.into());
                }
                1
            }
        };
        let grant = Grant {
            intent,
            claimant,
            epoch,
            phase: Phase::Recovering,
        };
        self.write_grant(transaction, grant).await?;
        Ok(Claim::Acquired(grant))
    }
}
