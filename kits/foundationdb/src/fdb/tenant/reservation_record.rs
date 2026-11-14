// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ReservationErrorReason, ReservationId, ReservationResult, TenantDeleteResolution};

const VERSION: u8 = 1;

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

impl StoredReservation {
    pub(super) fn encode(self) -> Vec<u8> {
        let phase = match self.phase {
            Phase::Deleting => 1,
            Phase::Recovering => 2,
            Phase::Complete(TenantDeleteResolution::OriginalAbsent) => 3,
            Phase::Complete(TenantDeleteResolution::MetadataSafe) => 4,
        };
        [
            &[VERSION, phase][..],
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
        if version != VERSION || original_id < 0 || epoch == 0 || expires_at <= 0 {
            return Err(invalid);
        }
        let phase = match phase {
            1 if epoch == 1 => Phase::Deleting,
            2 => Phase::Recovering,
            3 => Phase::Complete(TenantDeleteResolution::OriginalAbsent),
            4 => Phase::Complete(TenantDeleteResolution::MetadataSafe),
            _ => return Err(invalid),
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
