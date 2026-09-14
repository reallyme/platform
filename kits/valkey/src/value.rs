// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{ValkeyDataErrorReason, ValkeyDataKind, ValkeyError, ValkeyResult};

const MAX_KEY_BYTES: usize = 512;
const MAX_VALUE_BYTES: usize = 8 * 1_024 * 1_024;
const MAX_TTL_MILLIS: u64 = 604_800_000;

/// Validated binary key suffix.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct ValkeyKey(Vec<u8>);

impl ValkeyKey {
    /// Validates and constructs a binary key suffix.
    pub fn new(mut value: Vec<u8>) -> ValkeyResult<Self> {
        if value.is_empty() {
            value.zeroize();
            return Err(data_error(
                ValkeyDataKind::Key,
                ValkeyDataErrorReason::Empty,
            ));
        }
        if value.len() > MAX_KEY_BYTES {
            value.zeroize();
            return Err(data_error(
                ValkeyDataKind::Key,
                ValkeyDataErrorReason::TooLarge,
            ));
        }
        Ok(Self(value))
    }

    /// Returns the validated binary key suffix.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// Validated binary value that clears its allocation on drop.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct ValkeyValue(Vec<u8>);

impl ValkeyValue {
    /// Validates and constructs a bounded binary value.
    pub fn new(mut value: Vec<u8>) -> ValkeyResult<Self> {
        if value.len() > MAX_VALUE_BYTES {
            value.zeroize();
            return Err(data_error(
                ValkeyDataKind::Value,
                ValkeyDataErrorReason::TooLarge,
            ));
        }
        Ok(Self(value))
    }

    /// Returns the binary value.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// Positive, bounded key expiration interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValkeyTimeToLive {
    milliseconds: u64,
}

impl ValkeyTimeToLive {
    /// Constructs a TTL from a duration.
    pub fn new(value: Duration) -> ValkeyResult<Self> {
        let milliseconds = u64::try_from(value.as_millis()).map_err(|_| {
            data_error(
                ValkeyDataKind::TimeToLive,
                ValkeyDataErrorReason::OutOfRange,
            )
        })?;
        if milliseconds == 0 || milliseconds > MAX_TTL_MILLIS {
            return Err(data_error(
                ValkeyDataKind::TimeToLive,
                ValkeyDataErrorReason::OutOfRange,
            ));
        }
        Ok(Self { milliseconds })
    }

    /// Returns the TTL in whole milliseconds.
    pub const fn milliseconds(self) -> u64 {
        self.milliseconds
    }
}

const fn data_error(kind: ValkeyDataKind, reason: ValkeyDataErrorReason) -> ValkeyError {
    ValkeyError::InvalidData { kind, reason }
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
