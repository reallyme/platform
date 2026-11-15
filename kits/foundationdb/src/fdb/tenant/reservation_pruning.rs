// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use foundationdb::Transaction;

use super::{
    AttemptResult, CompletionTombstone, Phase, ReservationErrorReason, ReservationIntent,
    ReservationRecord, ReservationResult, ReservationStore, TenantDeleteResolution,
};

impl ReservationStore<'_> {
    /// Compacts a completed reservation in place and returns its saved outcome.
    /// An already compacted reservation returns the same outcome on retry.
    pub async fn prune_completed(
        &self,
        intent: ReservationIntent,
    ) -> ReservationResult<TenantDeleteResolution> {
        self.validate_intent(intent)?;
        self.transact(async |transaction| self.stage_prune(transaction, intent).await)
            .await
    }

    pub(super) async fn stage_prune(
        &self,
        transaction: &Transaction,
        intent: ReservationIntent,
    ) -> AttemptResult<TenantDeleteResolution> {
        let active = self.active_id(transaction, intent.tenant).await?;
        let record = self
            .read_record(transaction, intent.tenant, intent.operation)
            .await?
            .ok_or(ReservationErrorReason::PruneNotCompleted)?;
        let completion = match record {
            ReservationRecord::Tombstone(completion) => {
                return Self::completed_outcome(active, intent, completion);
            }
            ReservationRecord::Full(stored) => {
                if stored.original_id != intent.original_id
                    || stored.generation != intent.generation
                {
                    return Err(ReservationErrorReason::InvalidRecord.into());
                }
                let Phase::Complete(outcome) = stored.phase else {
                    return Err(ReservationErrorReason::PruneNotCompleted.into());
                };
                CompletionTombstone {
                    original_id: stored.original_id,
                    generation: stored.generation,
                    outcome,
                }
            }
        };
        let outcome = Self::completed_outcome(active, intent, completion)?;
        // Keep the same operation key occupied in the commit that drops the
        // grant. A missing key would let a pending local intent claim again.
        transaction.set(
            &self.record_key(intent.tenant, intent.operation),
            &completion.encode(),
        );
        Ok(outcome)
    }

    /// Removes an older tombstone after the durable cutoff excludes its intent.
    /// An already absent row below the cutoff is a successful retry.
    pub async fn reclaim_tombstone(&self, intent: ReservationIntent) -> ReservationResult<()> {
        self.validate_intent(intent)?;
        self.transact(async |transaction| self.stage_reclaim(transaction, intent).await)
            .await
    }

    pub(super) async fn stage_reclaim(
        &self,
        transaction: &Transaction,
        intent: ReservationIntent,
    ) -> AttemptResult<()> {
        if intent.generation >= self.read_cutoff(transaction, intent.tenant).await? {
            return Err(ReservationErrorReason::ReclaimNotEligible.into());
        }
        let active = self.active_id(transaction, intent.tenant).await?;
        if active == Some(intent.operation) {
            return Err(ReservationErrorReason::InvalidRecord.into());
        }
        match self
            .read_record(transaction, intent.tenant, intent.operation)
            .await?
        {
            None => Ok(()),
            Some(ReservationRecord::Full(_)) => {
                Err(ReservationErrorReason::ReclaimNotEligible.into())
            }
            Some(ReservationRecord::Tombstone(completion)) => {
                Self::completed_outcome(active, intent, completion)?;
                // Keep the cutoff and any newer grant intact. The original
                // generation will stay ineligible after this key is gone.
                transaction.clear(&self.record_key(intent.tenant, intent.operation));
                Ok(())
            }
        }
    }
}
