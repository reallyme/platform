// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use buffa::{DecodeOptions, Message};
use zeroize::Zeroizing;

#[cfg(feature = "tenant-admin")]
use crate::generated::reallyme::platform::storage::v1::{
    ActiveOperation, Completion, GenerationCutoff, OperatorDeleteRecord, Reservation,
};
use crate::generated::reallyme::platform::storage::v1::{CreationTime, SchemaVersion};

// The full header is longer than any legacy scalar or active nonce. The
// first byte is also disjoint from the legacy reservation version markers.
const FORMAT_PREFIX: &[u8] = b"\0reallyme-fdb-v1";
pub(crate) const MAX_PERSISTED_RECORD_BYTES: usize = 96;
const RECORD_RECURSION_LIMIT: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PersistedRecordError {
    #[error("invalid persisted protobuf record")]
    InvalidEncoding,
}

// Associate the discriminator with the message type to prevent an
// unrelated payload from being written under a different record tag.
pub(crate) trait PersistedMessage: Message {
    const TAG: u8;
    const MAX_BYTES: usize = MAX_PERSISTED_RECORD_BYTES;
}

impl PersistedMessage for SchemaVersion {
    const TAG: u8 = b'S';
}

impl PersistedMessage for CreationTime {
    const TAG: u8 = b'C';
}

#[cfg(feature = "tenant-admin")]
impl PersistedMessage for Reservation {
    const TAG: u8 = b'R';
}

#[cfg(feature = "tenant-admin")]
impl PersistedMessage for Completion {
    const TAG: u8 = b'T';
}

#[cfg(feature = "tenant-admin")]
impl PersistedMessage for ActiveOperation {
    const TAG: u8 = b'A';
}

#[cfg(feature = "tenant-admin")]
impl PersistedMessage for GenerationCutoff {
    const TAG: u8 = b'G';
}

#[cfg(feature = "tenant-admin")]
impl PersistedMessage for OperatorDeleteRecord {
    const TAG: u8 = b'I';
    const MAX_BYTES: usize = super::fdb::tenant::admin::delete_intent::MAX_DELETE_RECORD_BYTES;
}

pub(crate) fn encode_record<M: PersistedMessage>(message: &M) -> Zeroizing<Vec<u8>> {
    // Reserve for the full validated message before writing so reallocation
    // cannot leave a copy of its fields in the allocator. A decoded message
    // came from an input already bounded by this same cap.
    let mut bytes = Zeroizing::new(Vec::with_capacity(M::MAX_BYTES));
    bytes.extend_from_slice(FORMAT_PREFIX);
    bytes.push(M::TAG);
    message.encode(&mut *bytes);
    bytes
}

pub(crate) fn decode_record<M: PersistedMessage>(bytes: &[u8]) -> Result<M, PersistedRecordError> {
    let invalid = PersistedRecordError::InvalidEncoding;
    if bytes.len() > M::MAX_BYTES {
        return Err(invalid);
    }
    let body = bytes.strip_prefix(FORMAT_PREFIX).ok_or(invalid)?;
    let (&tag, payload) = body.split_first().ok_or(invalid)?;
    if tag != M::TAG {
        return Err(invalid);
    }
    let message = DecodeOptions::new()
        .reject_unknown_fields()
        .with_recursion_limit(RECORD_RECURSION_LIMIT)
        .decode_from_slice(payload)
        .map_err(|_| invalid)?;
    // Only this repository writes these records. Reject duplicate fields and
    // noncanonical encodings before they can become trusted lease metadata.
    if encode_record(&message).as_slice() != bytes {
        return Err(invalid);
    }
    Ok(message)
}

#[cfg(feature = "tenant-admin")]
pub(crate) fn has_record_kind<M: PersistedMessage>(bytes: &[u8]) -> bool {
    bytes
        .strip_prefix(FORMAT_PREFIX)
        .is_some_and(|rest| rest.first() == Some(&M::TAG))
}

#[cfg(test)]
#[path = "persisted_record_tests.rs"]
mod tests;
