// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::validate_static_key_label;
use crate::keys::error::KeyLabelError;

#[test]
fn accepts_stable_key_labels() {
    assert!(validate_static_key_label("tenant-1.core").is_ok());
}

#[test]
fn rejects_empty_key_labels() {
    assert!(matches!(
        validate_static_key_label(""),
        Err(KeyLabelError::Empty)
    ));
}

#[test]
fn rejects_ambiguous_key_label_bytes() {
    assert!(matches!(
        validate_static_key_label("Tenant/One"),
        Err(KeyLabelError::InvalidByte)
    ));
}
