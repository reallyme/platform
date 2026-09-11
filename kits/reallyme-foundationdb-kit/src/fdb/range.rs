// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Range helpers for FoundationDB scans.

use crate::fdb::error::{FdbError, FdbQueryErrorReason, FdbResult};
use bytes::{Bytes, BytesMut};

/// Computes the exclusive range end for a prefix scan.
///
/// This legacy API returns an owned `Vec<u8>` for existing call sites and older
/// adapters. Prefer [`range_end_for_prefix_bytes`] in new or performance-critical
/// paths so key buffers stay in `bytes`-owned form end-to-end.
pub fn range_end_for_prefix(prefix: &[u8]) -> FdbResult<Vec<u8>> {
    range_end_for_prefix_bytes(prefix).map(|value| value.to_vec())
}

/// Computes the exclusive range end for a prefix scan using a `bytes` key buffer.
///
/// The caller receives `Bytes`, which aligns with this kit's preference for key
/// ownership and reduces conversion churn in hot scan paths.
pub fn range_end_for_prefix_bytes(prefix: &[u8]) -> FdbResult<Bytes> {
    if prefix.is_empty() {
        return Err(FdbError::Query {
            reason: FdbQueryErrorReason::InvalidRangePrefix,
        });
    }

    let mut end = BytesMut::from(prefix);
    for index in (0..end.len()).rev() {
        if end[index] != 0xff {
            end[index] = end[index].checked_add(1).ok_or(FdbError::Query {
                reason: FdbQueryErrorReason::IntegerOverflow,
            })?;
            end.truncate(index.checked_add(1).ok_or(FdbError::Query {
                reason: FdbQueryErrorReason::IntegerOverflow,
            })?);
            return Ok(end.freeze());
        }
    }

    Err(FdbError::Query {
        reason: FdbQueryErrorReason::InvalidRangePrefix,
    })
}

#[cfg(test)]
mod tests {
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
}
