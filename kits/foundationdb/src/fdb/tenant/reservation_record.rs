// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ReservationErrorReason, ReservationId, ReservationResult, TenantDeleteResolution};

const FULL_VERSION: u8 = 1;
const TOMBSTONE_VERSION: u8 = 2;
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
    pub(super) epoch: u64,
    pub(super) claimant: ReservationId,
    pub(super) expires_at: i64,
    pub(super) phase: Phase,
}

#[derive(Clone, Copy)]
pub(super) struct CompletionTombstone {
    pub(super) original_id: i64,
    pub(super) outcome: TenantDeleteResolution,
}

pub(super) enum ReservationRecord {
    Full(StoredReservation),
    Tombstone(CompletionTombstone),
}

impl ReservationRecord {
    pub(super) fn decode(bytes: &[u8]) -> ReservationResult<Self> {
        match bytes.first().copied() {
            Some(FULL_VERSION) => StoredReservation::decode(bytes).map(Self::Full),
            Some(TOMBSTONE_VERSION) => CompletionTombstone::decode(bytes).map(Self::Tombstone),
            _ => Err(ReservationErrorReason::InvalidRecord),
        }
    }
}

impl CompletionTombstone {
    pub(super) fn encode(self) -> [u8; TOMBSTONE_BYTES] {
        let mut bytes = [0; TOMBSTONE_BYTES];
        let [version, outcome, id @ ..] = &mut bytes;
        *version = TOMBSTONE_VERSION;
        *outcome = encode_outcome(self.outcome);
        id.copy_from_slice(&self.original_id.to_be_bytes());
        bytes
    }

    fn decode(bytes: &[u8]) -> ReservationResult<Self> {
        let invalid = ReservationErrorReason::InvalidRecord;
        let &[TOMBSTONE_VERSION, outcome, id @ ..] =
            <&[u8; TOMBSTONE_BYTES]>::try_from(bytes).map_err(|_| invalid)?
        else {
            return Err(invalid);
        };
        let original_id = i64::from_be_bytes(id);
        if original_id < 0 {
            return Err(invalid);
        }
        Ok(Self {
            original_id,
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
        [
            &[FULL_VERSION, phase][..],
            &self.original_id.to_be_bytes(),
            &self.epoch.to_be_bytes(),
            self.claimant.as_bytes(),
            &self.expires_at.to_be_bytes(),
        ]
        .concat()
    }

    pub(super) fn decode(bytes: &[u8]) -> ReservationResult<Self> {
        let invalid = ReservationErrorReason::InvalidRecord;
        let (&[version, phase], bytes) = bytes.split_first_chunk().ok_or(invalid)?;
        let (id, bytes) = bytes.split_first_chunk().ok_or(invalid)?;
        let (epoch, bytes) = bytes.split_first_chunk().ok_or(invalid)?;
        let (claimant, expires_at) = bytes.split_first_chunk().ok_or(invalid)?;
        let original_id = i64::from_be_bytes(*id);
        let epoch = u64::from_be_bytes(*epoch);
        let expires_at = i64::from_be_bytes(expires_at.try_into().map_err(|_| invalid)?);
        if version != FULL_VERSION || original_id < 0 || epoch == 0 || expires_at <= 0 {
            return Err(invalid);
        }
        let phase = match phase {
            1 if epoch == 1 => Phase::Deleting,
            2 => Phase::Recovering,
            _ => Phase::Complete(decode_outcome(phase)?),
        };
        Ok(Self {
            original_id,
            epoch,
            claimant: ReservationId::from_bytes(*claimant),
            expires_at,
            phase,
        })
    }
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
