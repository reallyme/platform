// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::{ValkeyKey, ValkeyTimeToLive, ValkeyValue};
use crate::{ValkeyDataErrorReason, ValkeyDataKind, ValkeyError};

#[test]
fn key_rejects_empty_input() {
    assert_eq!(
        ValkeyKey::new(Vec::new()).err(),
        Some(ValkeyError::InvalidData {
            kind: ValkeyDataKind::Key,
            reason: ValkeyDataErrorReason::Empty,
        })
    );
}

#[test]
fn value_accepts_empty_payload_but_rejects_oversized_payload() {
    assert!(ValkeyValue::new(Vec::new()).is_ok());
    let oversized = vec![0_u8; 8 * 1_024 * 1_024 + 1];
    assert_eq!(
        ValkeyValue::new(oversized).err(),
        Some(ValkeyError::InvalidData {
            kind: ValkeyDataKind::Value,
            reason: ValkeyDataErrorReason::TooLarge,
        })
    );
}

#[test]
fn ttl_rejects_zero_and_unbounded_values() {
    assert!(ValkeyTimeToLive::new(Duration::ZERO).is_err());
    assert!(ValkeyTimeToLive::new(Duration::from_secs(604_801)).is_err());
    assert_eq!(
        ValkeyTimeToLive::new(Duration::from_secs(60))
            .expect("TTL fixture should be valid")
            .milliseconds(),
        60_000
    );
}
