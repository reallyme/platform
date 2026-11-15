// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::ops::Deref;
use std::str;

use buffa::{DecodeError, FieldPayload, ProtoString, WirePayload};
use bytes::Buf;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::MAX_FOUNDATIONDB_TENANT_NAME_LENGTH;

#[derive(Clone, Default, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub(crate) struct StoredTenantName(Zeroizing<String>);

impl fmt::Debug for StoredTenantName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

impl AsRef<str> for StoredTenantName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Deref for StoredTenantName {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_ref()
    }
}

impl From<String> for StoredTenantName {
    fn from(value: String) -> Self {
        let value = Zeroizing::new(value);
        if value.len() > MAX_FOUNDATIONDB_TENANT_NAME_LENGTH {
            return Self::default();
        }
        Self(value)
    }
}

impl ProtoString for StoredTenantName {
    fn copy_from_str(value: &str) -> Self {
        if value.len() > MAX_FOUNDATIONDB_TENANT_NAME_LENGTH {
            return Self::default();
        }
        Self(Zeroizing::new(value.to_owned()))
    }

    fn from_wire(payload: WirePayload<'_>) -> Result<Self, DecodeError> {
        check_length(payload.as_slice().len())?;
        payload.to_str().map(Self::copy_from_str)
    }

    fn decode_length_delimited(payload: &mut FieldPayload<impl Buf>) -> Result<Self, DecodeError> {
        let length = payload.remaining();
        check_length(length)?;
        // Fragmented names pass through bounded wiping storage before UTF-8
        // validation. The final copy owns its allocation from creation.
        let mut bytes = Zeroizing::new([0; MAX_FOUNDATIONDB_TENANT_NAME_LENGTH]);
        let bytes = &mut bytes[..length];
        payload.copy_to_slice(bytes);
        let value = str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
        Ok(Self::copy_from_str(value))
    }
}

fn check_length(length: usize) -> Result<(), DecodeError> {
    if length > MAX_FOUNDATIONDB_TENANT_NAME_LENGTH {
        return Err(DecodeError::MessageTooLarge);
    }
    Ok(())
}
