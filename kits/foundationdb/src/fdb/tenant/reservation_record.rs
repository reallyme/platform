// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use zeroize::Zeroizing;

use crate::generated::reallyme::platform::storage::v1::{
    Completion, DeleteOutcome, Reservation, ReservationPhase,
};
use crate::persisted_record::{decode_record, encode_record, has_record_kind};
use crate::stored_id::StoredId;

use super::{
    ReservationErrorReason, ReservationGeneration, ReservationId, ReservationResult,
    TenantDeleteResolution,
};

const FULL_VERSION: u8 = 1;
const TOMBSTONE_VERSION: u8 = 2;
const STAMPED_FULL_VERSION: u8 = 3;
const STAMPED_TOMBSTONE_VERSION: u8 = 4;
const ORIGINAL_ABSENT: u8 = 3;
const METADATA_SAFE: u8 = 4;
const TOMBSTONE_BYTES: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Deleting,
    Recovering,
    Complete(TenantDeleteResolution),
}

#[derive(Clone, Copy)]
pub(super) struct StoredReservation {
    pub(super) original_id: i64,
    pub(super) generation: ReservationGeneration,
    pub(super) epoch: u64,
    pub(super) claimant: ReservationId,
    pub(super) expires_at: i64,
    pub(super) phase: Phase,
}

#[derive(Clone, Copy)]
pub(super) struct CompletionTombstone {
    pub(super) original_id: i64,
    pub(super) generation: ReservationGeneration,
    pub(super) outcome: TenantDeleteResolution,
}

pub(super) enum ReservationRecord {
    Full(StoredReservation),
    Tombstone(CompletionTombstone),
}

impl ReservationRecord {
    pub(super) fn decode(bytes: &[u8]) -> ReservationResult<Self> {
        match bytes.first().copied() {
            Some(FULL_VERSION | STAMPED_FULL_VERSION) => {
                StoredReservation::decode(bytes).map(Self::Full)
            }
            Some(TOMBSTONE_VERSION | STAMPED_TOMBSTONE_VERSION) => {
                CompletionTombstone::decode(bytes).map(Self::Tombstone)
            }
            _ if has_record_kind::<Reservation>(bytes) => {
                StoredReservation::decode_proto(bytes).map(Self::Full)
            }
            _ if has_record_kind::<Completion>(bytes) => {
                CompletionTombstone::decode_proto(bytes).map(Self::Tombstone)
            }
            _ => Err(ReservationErrorReason::InvalidRecord),
        }
    }
}

impl CompletionTombstone {
    pub(super) fn encode(self) -> Zeroizing<Vec<u8>> {
        let outcome = match self.outcome {
            TenantDeleteResolution::OriginalAbsent => DeleteOutcome::OriginalAbsent,
            TenantDeleteResolution::MetadataSafe => DeleteOutcome::MetadataSafe,
        };
        let message = Completion::default()
            .with_original_id(self.original_id)
            .with_generation(self.generation.value())
            .with_outcome(outcome);
        encode_record(&message)
    }

    fn decode_proto(bytes: &[u8]) -> ReservationResult<Self> {
        let invalid = ReservationErrorReason::InvalidRecord;
        let message: Completion = decode_record(bytes).map_err(|_| invalid)?;
        let original_id = message.original_id.filter(|id| *id >= 0).ok_or(invalid)?;
        let outcome = match message.outcome.as_known() {
            Some(DeleteOutcome::OriginalAbsent) => TenantDeleteResolution::OriginalAbsent,
            Some(DeleteOutcome::MetadataSafe) => TenantDeleteResolution::MetadataSafe,
            _ => return Err(invalid),
        };
        Ok(Self {
            original_id,
            generation: ReservationGeneration::new(message.generation),
            outcome,
        })
    }

    fn decode(bytes: &[u8]) -> ReservationResult<Self> {
        let invalid = ReservationErrorReason::InvalidRecord;
        let (bytes, generation) =
            split_generation(bytes, TOMBSTONE_VERSION, STAMPED_TOMBSTONE_VERSION)?;
        let &[_, outcome, id @ ..] =
            <&[u8; TOMBSTONE_BYTES]>::try_from(bytes).map_err(|_| invalid)?;
        let original_id = i64::from_be_bytes(id);
        if original_id < 0 {
            return Err(invalid);
        }
        Ok(Self {
            original_id,
            generation,
            outcome: decode_outcome(outcome)?,
        })
    }
}

impl StoredReservation {
    pub(super) fn encode(self) -> Zeroizing<Vec<u8>> {
        let phase = match self.phase {
            Phase::Deleting => ReservationPhase::Deleting,
            Phase::Recovering => ReservationPhase::Recovering,
            Phase::Complete(TenantDeleteResolution::OriginalAbsent) => {
                ReservationPhase::OriginalAbsent
            }
            Phase::Complete(TenantDeleteResolution::MetadataSafe) => ReservationPhase::MetadataSafe,
        };
        let message = Reservation::default()
            .with_original_id(self.original_id)
            .with_generation(self.generation.value())
            .with_epoch(self.epoch)
            .with_claimant(StoredId::new(*self.claimant.as_bytes()))
            .with_expires_at(self.expires_at)
            .with_phase(phase);
        encode_record(&message)
    }

    fn decode_proto(bytes: &[u8]) -> ReservationResult<Self> {
        let invalid = ReservationErrorReason::InvalidRecord;
        let message: Reservation = decode_record(bytes).map_err(|_| invalid)?;
        let original_id = message.original_id.filter(|id| *id >= 0).ok_or(invalid)?;
        let claimant = message.claimant.bytes().ok_or(invalid)?;
        if message.epoch == 0 || message.expires_at <= 0 {
            return Err(invalid);
        }
        let phase = match message.phase.as_known() {
            Some(ReservationPhase::Deleting) if message.epoch == 1 => Phase::Deleting,
            Some(ReservationPhase::Recovering) => Phase::Recovering,
            Some(ReservationPhase::OriginalAbsent) => {
                Phase::Complete(TenantDeleteResolution::OriginalAbsent)
            }
            Some(ReservationPhase::MetadataSafe) => {
                Phase::Complete(TenantDeleteResolution::MetadataSafe)
            }
            _ => return Err(invalid),
        };
        Ok(Self {
            original_id,
            generation: ReservationGeneration::new(message.generation),
            epoch: message.epoch,
            claimant: ReservationId::from_bytes(*claimant),
            expires_at: message.expires_at,
            phase,
        })
    }

    pub(super) fn decode(bytes: &[u8]) -> ReservationResult<Self> {
        if has_record_kind::<Reservation>(bytes) {
            return Self::decode_proto(bytes);
        }
        let invalid = ReservationErrorReason::InvalidRecord;
        let (bytes, generation) = split_generation(bytes, FULL_VERSION, STAMPED_FULL_VERSION)?;
        let (&[_, phase], bytes) = bytes.split_first_chunk().ok_or(invalid)?;
        let (id, bytes) = bytes.split_first_chunk().ok_or(invalid)?;
        let (epoch, bytes) = bytes.split_first_chunk().ok_or(invalid)?;
        let (claimant, expires_at) = bytes.split_first_chunk().ok_or(invalid)?;
        let original_id = i64::from_be_bytes(*id);
        let epoch = u64::from_be_bytes(*epoch);
        let expires_at = i64::from_be_bytes(expires_at.try_into().map_err(|_| invalid)?);
        if original_id < 0 || epoch == 0 || expires_at <= 0 {
            return Err(invalid);
        }
        let phase = match phase {
            1 if epoch == 1 => Phase::Deleting,
            2 => Phase::Recovering,
            _ => Phase::Complete(decode_outcome(phase)?),
        };
        Ok(Self {
            original_id,
            generation,
            epoch,
            claimant: ReservationId::from_bytes(*claimant),
            expires_at,
            phase,
        })
    }
}

fn split_generation(
    bytes: &[u8],
    legacy: u8,
    stamped: u8,
) -> ReservationResult<(&[u8], ReservationGeneration)> {
    let invalid = ReservationErrorReason::InvalidRecord;
    let version = bytes.first().copied().ok_or(invalid)?;
    if version == legacy {
        return Ok((bytes, ReservationGeneration::LEGACY));
    }
    if version != stamped {
        return Err(invalid);
    }
    let (body, generation) = bytes.split_last_chunk().ok_or(invalid)?;
    let generation = ReservationGeneration::new(u64::from_be_bytes(*generation));
    if generation == ReservationGeneration::LEGACY {
        return Err(invalid);
    }
    Ok((body, generation))
}

const fn decode_outcome(outcome: u8) -> ReservationResult<TenantDeleteResolution> {
    match outcome {
        ORIGINAL_ABSENT => Ok(TenantDeleteResolution::OriginalAbsent),
        METADATA_SAFE => Ok(TenantDeleteResolution::MetadataSafe),
        _ => Err(ReservationErrorReason::InvalidRecord),
    }
}
