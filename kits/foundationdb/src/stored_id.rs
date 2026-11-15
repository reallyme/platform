// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::ops::Deref;

use buffa::{DecodeError, FieldPayload, ProtoBytes, WirePayload};
use bytes::Buf;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub(crate) type StoredId = StoredIdentity<16>;
pub(crate) type StoredClusterId = StoredIdentity<32>;

/// A fixed-width opaque identity decoded by the generated storage messages.
#[derive(Clone, Default, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub(crate) struct StoredIdentity<const N: usize>(Option<[u8; N]>);

impl<const N: usize> StoredIdentity<N> {
    #[cfg(feature = "tenant-admin")]
    pub(crate) const fn new(value: [u8; N]) -> Self {
        Self(Some(value))
    }

    #[cfg(feature = "tenant-admin")]
    pub(crate) const fn bytes(&self) -> Option<&[u8; N]> {
        self.0.as_ref()
    }
}

impl<const N: usize> fmt::Debug for StoredIdentity<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

impl<const N: usize> Deref for StoredIdentity<N> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}

impl<const N: usize> AsRef<[u8]> for StoredIdentity<N> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref().map_or(&[], |value| value.as_slice())
    }
}

impl<const N: usize> From<Vec<u8>> for StoredIdentity<N> {
    fn from(value: Vec<u8>) -> Self {
        let value = Zeroizing::new(value);
        // Buffa requires an infallible conversion. The absent value still
        // fails record validation and cannot become a usable nonce.
        Self(value.as_slice().try_into().ok())
    }
}

impl<const N: usize> ProtoBytes for StoredIdentity<N> {
    fn from_wire(payload: WirePayload<'_>) -> Result<Self, DecodeError> {
        let bytes = payload.as_slice();
        check_length(bytes.len(), N)?;
        let mut result = Zeroizing::new([0; N]);
        result.copy_from_slice(bytes);
        Ok(Self(Some(*result)))
    }

    fn decode_length_delimited(payload: &mut FieldPayload<impl Buf>) -> Result<Self, DecodeError> {
        check_length(payload.remaining(), N)?;
        // The exact-length check makes the read bounded. Copy directly into
        // wiping storage even when the field spans several input chunks.
        let mut bytes = Zeroizing::new([0; N]);
        payload.copy_to_slice(&mut *bytes);
        Ok(Self(Some(*bytes)))
    }
}

fn check_length(length: usize, expected: usize) -> Result<(), DecodeError> {
    match length.cmp(&expected) {
        std::cmp::Ordering::Less => Err(DecodeError::UnexpectedEof),
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Greater => Err(DecodeError::MessageTooLarge),
    }
}
