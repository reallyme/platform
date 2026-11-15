// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed record encoding for the companion tenant-admin binary.

use buffa::ProtoString;
use zeroize::Zeroizing;

use super::TenantAdminClusterId;
use super::reservations::{ReservationControlTenant, ReservationGeneration, ReservationId};
use crate::FoundationDbTenantName;
use crate::generated::reallyme::platform::storage::v1::{
    OperatorDeleteRecord, OperatorDeleteReservation, OperatorDeleteState,
};
use crate::persisted_record::{decode_record, encode_record};
use crate::stored_id::{StoredClusterId, StoredId};
use crate::stored_tenant_name::StoredTenantName;

/// Maximum size of an intent file, including the format header.
pub const MAX_DELETE_RECORD_BYTES: usize = 320;

/// A durable intent could not be represented or validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DeleteIntentError {
    /// The record is malformed or violates the operator recovery invariants.
    #[error("invalid durable delete intent")]
    InvalidRecord,
}

/// The operator's progress on a durable intent.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DeleteRecordState {
    /// The outcome has yet to be reconciled.
    Pending,
    /// The recorded tenant ID no longer owns the name.
    OriginalAbsent,
    /// The recorded tenant exists with compatible metadata.
    MetadataSafe,
}

impl DeleteRecordState {
    /// The CLI output for this state.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::OriginalAbsent => "original-absent",
            Self::MetadataSafe => "metadata-safe",
        }
    }
}

/// Identity and progress persisted before starting tenant deletion.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DeleteRecord {
    /// The cluster that owns the original tenant ID.
    pub cluster: TenantAdminClusterId,
    /// The target tenant name.
    pub tenant: FoundationDbTenantName,
    /// The target tenant ID captured before deletion.
    pub original_id: i64,
    /// The latest durable state of the intent.
    pub state: DeleteRecordState,
    /// The control-tenant reservation when coordination is enabled.
    pub reservation: Option<DeleteReservation>,
}

/// The control-tenant reservation associated with a durable intent.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DeleteReservation {
    /// The control tenant that owns this reservation.
    pub control: ReservationControlTenant,
    /// The opaque operation identity.
    pub operation: ReservationId,
    /// The admission generation captured when the intent was created.
    pub generation: ReservationGeneration,
}

impl DeleteRecord {
    /// Decodes a complete, canonical record and validates its identities.
    pub fn decode_protobuf(bytes: &[u8]) -> Result<Self, DeleteIntentError> {
        let invalid = DeleteIntentError::InvalidRecord;
        let wire: OperatorDeleteRecord = decode_record(bytes).map_err(|_| invalid)?;
        let uid = wire.cluster_uid.bytes().ok_or(invalid)?;
        let cluster = TenantAdminClusterId::from_bytes(uid).ok_or(invalid)?;
        if cluster.as_bytes() != uid {
            return Err(invalid);
        }
        let tenant = FoundationDbTenantName::new(&wire.tenant_name).map_err(|_| invalid)?;
        let original_id = wire.original_id.filter(|id| *id >= 0).ok_or(invalid)?;
        let state = match wire.state.as_known() {
            Some(OperatorDeleteState::Pending) => DeleteRecordState::Pending,
            Some(OperatorDeleteState::OriginalAbsent) => DeleteRecordState::OriginalAbsent,
            Some(OperatorDeleteState::MetadataSafe) => DeleteRecordState::MetadataSafe,
            _ => return Err(invalid),
        };
        let reservation = wire
            .reservation
            .as_option()
            .map(decode_reservation)
            .transpose()?;
        let record = Self {
            cluster,
            tenant,
            original_id,
            state,
            reservation,
        };
        record.validate()?;
        Ok(record)
    }

    /// Encodes validated state into a buffer that wipes on drop.
    pub fn encode_protobuf(&self) -> Result<Zeroizing<Vec<u8>>, DeleteIntentError> {
        self.validate()?;
        let state = match self.state {
            DeleteRecordState::Pending => OperatorDeleteState::Pending,
            DeleteRecordState::OriginalAbsent => OperatorDeleteState::OriginalAbsent,
            DeleteRecordState::MetadataSafe => OperatorDeleteState::MetadataSafe,
        };
        let mut wire = OperatorDeleteRecord::default()
            .with_cluster_uid(StoredClusterId::new(*self.cluster.as_bytes()))
            .with_tenant_name(encode_name(self.tenant)?)
            .with_original_id(self.original_id)
            .with_state(state);
        if let Some(reservation) = self.reservation {
            wire = wire.with_reservation(
                OperatorDeleteReservation::default()
                    .with_control_name(encode_name(reservation.control.name())?)
                    .with_control_id(reservation.control.id())
                    .with_operation(StoredId::new(*reservation.operation.as_bytes()))
                    .with_generation(reservation.generation.value()),
            );
        }
        Ok(encode_record(&wire))
    }

    fn validate(&self) -> Result<(), DeleteIntentError> {
        if self.original_id < 0
            || self.reservation.is_some_and(|reservation| {
                reservation.control.name() == self.tenant
                    || reservation.control.id() == self.original_id
            })
        {
            return Err(DeleteIntentError::InvalidRecord);
        }
        Ok(())
    }
}

fn encode_name(name: FoundationDbTenantName) -> Result<StoredTenantName, DeleteIntentError> {
    let name =
        std::str::from_utf8(name.as_bytes()).map_err(|_| DeleteIntentError::InvalidRecord)?;
    Ok(StoredTenantName::copy_from_str(name))
}

fn decode_reservation(
    wire: &OperatorDeleteReservation,
) -> Result<DeleteReservation, DeleteIntentError> {
    let invalid = DeleteIntentError::InvalidRecord;
    let name = FoundationDbTenantName::new(&wire.control_name).map_err(|_| invalid)?;
    let control = ReservationControlTenant::new(name, wire.control_id.ok_or(invalid)?)
        .map_err(|_| invalid)?;
    let operation = ReservationId::from_bytes(*wire.operation.bytes().ok_or(invalid)?);
    Ok(DeleteReservation {
        control,
        operation,
        generation: ReservationGeneration::new(wire.generation),
    })
}
