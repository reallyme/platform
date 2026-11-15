// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

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
            _ => Err(ReservationErrorReason::InvalidRecord),
        }
    }
}

impl CompletionTombstone {
    pub(super) fn encode(self) -> Vec<u8> {
        let mut bytes = [0; TOMBSTONE_BYTES];
        let [version, outcome, id @ ..] = &mut bytes;
        *version = version_for(
            self.generation,
            TOMBSTONE_VERSION,
            STAMPED_TOMBSTONE_VERSION,
        );
        *outcome = encode_outcome(self.outcome);
        id.copy_from_slice(&self.original_id.to_be_bytes());
        let mut bytes = bytes.to_vec();
        append_generation(&mut bytes, self.generation);
        bytes
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
    pub(super) fn encode(self) -> Vec<u8> {
        let phase = match self.phase {
            Phase::Deleting => 1,
            Phase::Recovering => 2,
            Phase::Complete(outcome) => encode_outcome(outcome),
        };
        let version = version_for(self.generation, FULL_VERSION, STAMPED_FULL_VERSION);
        let mut bytes = [
            &[version, phase][..],
            &self.original_id.to_be_bytes(),
            &self.epoch.to_be_bytes(),
            self.claimant.as_bytes(),
            &self.expires_at.to_be_bytes(),
        ]
        .concat();
        append_generation(&mut bytes, self.generation);
        bytes
    }

    pub(super) fn decode(bytes: &[u8]) -> ReservationResult<Self> {
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

const fn version_for(generation: ReservationGeneration, legacy: u8, stamped: u8) -> u8 {
    if generation.value() == 0 {
        legacy
    } else {
        stamped
    }
}

fn append_generation(bytes: &mut Vec<u8>, generation: ReservationGeneration) {
    if generation != ReservationGeneration::LEGACY {
        bytes.extend_from_slice(&generation.value().to_be_bytes());
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

const fn encode_outcome(outcome: TenantDeleteResolution) -> u8 {
    match outcome {
        TenantDeleteResolution::OriginalAbsent => ORIGINAL_ABSENT,
        TenantDeleteResolution::MetadataSafe => METADATA_SAFE,
    }
}

const fn decode_outcome(outcome: u8) -> ReservationResult<TenantDeleteResolution> {
    match outcome {
        ORIGINAL_ABSENT => Ok(TenantDeleteResolution::OriginalAbsent),
        METADATA_SAFE => Ok(TenantDeleteResolution::MetadataSafe),
        _ => Err(ReservationErrorReason::InvalidRecord),
    }
}
