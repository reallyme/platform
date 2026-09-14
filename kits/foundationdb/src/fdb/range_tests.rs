// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::range_end_for_prefix;
use super::range_end_for_prefix_bytes;

#[test]
fn prefix_range_end_increments_last_non_max_byte() {
    let computed = range_end_for_prefix(&[0x10, 0x20]);
    assert!(computed.is_ok());
    assert_eq!(
        computed.unwrap_or_else(|_| unreachable!()),
        vec![0x10, 0x21]
    );
}

#[test]
fn prefix_range_end_carries_trailing_max_bytes() {
    let computed = range_end_for_prefix(&[0x10, 0xff]);
    assert!(computed.is_ok());
    assert_eq!(computed.unwrap_or_else(|_| unreachable!()), vec![0x11]);
}

#[test]
fn prefix_range_end_rejects_empty_prefix() {
    assert!(range_end_for_prefix(&[]).is_err());
}

#[test]
fn prefix_range_end_bytes_prefers_bytes_buffer() {
    let computed = range_end_for_prefix_bytes(&[0x10, 0x20]);
    assert!(computed.is_ok());
    assert_eq!(
        computed.unwrap_or_else(|_| unreachable!()).as_ref(),
        [0x10, 0x21]
    );
}
