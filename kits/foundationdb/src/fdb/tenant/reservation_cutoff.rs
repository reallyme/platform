// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cmp::Ordering;

use foundationdb::Transaction;

use crate::generated::reallyme::platform::storage::v1::GenerationCutoff;
use crate::persisted_record::{decode_record, encode_record};

use super::{
    AttemptResult, NAMESPACE, ReservationErrorReason, ReservationGeneration, ReservationId,
    ReservationIntent, ReservationResult, ReservationStore, transactions::read_target_id,
};
use crate::FoundationDbTenantName;

impl ReservationStore<'_> {
    /// Captures the tenant ID and generation before the caller persists its intent.
    pub async fn capture_intent(
        &self,
        tenant: FoundationDbTenantName,
    ) -> ReservationResult<Option<ReservationIntent>> {
        self.validate_target(tenant)?;
        let operation = ReservationId::generate()?;
        self.transact(async |transaction| {
            let generation = self.read_cutoff(transaction, tenant).await?;
            let Some(original_id) = read_target_id(transaction, tenant).await? else {
                return Ok(None);
            };
            let intent = ReservationIntent {
                tenant,
                original_id,
                operation,
                generation,
            };
            self.validate_intent(intent)?;
            Ok(Some(intent))
        })
        .await
    }

    /// Reads the current admission cutoff for a tenant name.
    pub async fn cutoff(
        &self,
        tenant: FoundationDbTenantName,
    ) -> ReservationResult<ReservationGeneration> {
        self.validate_target(tenant)?;
        self.transact(async |transaction| self.read_cutoff(transaction, tenant).await)
            .await
    }

    /// Advances to an explicit generation when the tenant name has no active grant.
    /// Retrying an applied target returns the current cutoff without moving it again.
    pub async fn advance_cutoff(
        &self,
        tenant: FoundationDbTenantName,
        target: ReservationGeneration,
    ) -> ReservationResult<ReservationGeneration> {
        self.validate_target(tenant)?;
        self.transact(async |transaction| {
            self.stage_advance_cutoff(transaction, tenant, target).await
        })
        .await
    }

    pub(super) async fn stage_advance_cutoff(
        &self,
        transaction: &Transaction,
        tenant: FoundationDbTenantName,
        target: ReservationGeneration,
    ) -> AttemptResult<ReservationGeneration> {
        let invalid = ReservationErrorReason::InvalidCutoffTarget;
        if target == ReservationGeneration::LEGACY {
            return Err(invalid.into());
        }
        let current = self.read_cutoff(transaction, tenant).await?;
        if current >= target {
            return Ok(current);
        }
        if current.value().checked_add(1) != Some(target.value()) {
            return Err(invalid.into());
        }
        // This read conflicts with a concurrent grant. Conversely, each grant
        // reads the cutoff so it cannot commit with an earlier generation.
        if self.active_id(transaction, tenant).await?.is_some() {
            return Err(ReservationErrorReason::Busy.into());
        }
        let cutoff = GenerationCutoff::default().with_generation(target.value());
        transaction.set(&self.cutoff_key(tenant), &encode_record(&cutoff));
        Ok(target)
    }

    pub(super) async fn check_cutoff(
        &self,
        transaction: &Transaction,
        intent: ReservationIntent,
    ) -> AttemptResult<()> {
        match intent
            .generation
            .cmp(&self.read_cutoff(transaction, intent.tenant).await?)
        {
            Ordering::Less => Err(ReservationErrorReason::IntentExpired.into()),
            Ordering::Equal => Ok(()),
            Ordering::Greater => Err(ReservationErrorReason::InvalidRecord.into()),
        }
    }

    pub(super) async fn read_cutoff(
        &self,
        transaction: &Transaction,
        tenant: FoundationDbTenantName,
    ) -> AttemptResult<ReservationGeneration> {
        let Some(value) = transaction.get(&self.cutoff_key(tenant), false).await? else {
            return Ok(ReservationGeneration::LEGACY);
        };
        let generation = if let Ok(legacy) = value.as_ref().try_into() {
            u64::from_be_bytes(legacy)
        } else {
            let record: GenerationCutoff =
                decode_record(value.as_ref()).map_err(|_| ReservationErrorReason::InvalidRecord)?;
            record.generation
        };
        let generation = ReservationGeneration::new(generation);
        if generation == ReservationGeneration::LEGACY {
            return Err(ReservationErrorReason::InvalidRecord.into());
        }
        Ok(generation)
    }

    pub(super) fn cutoff_key(&self, tenant: FoundationDbTenantName) -> Vec<u8> {
        [
            self.control.id().to_be_bytes().as_slice(),
            NAMESPACE,
            b"cutoff/",
            tenant.as_bytes(),
        ]
        .concat()
    }
}
