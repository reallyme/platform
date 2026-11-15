// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

pub(crate) const RESERVED_METADATA_START: &[u8] = b"__meta";
pub(crate) const RESERVED_METADATA_END: &[u8] = b"__metb";

pub(super) fn is_reserved_key(bytes: &[u8]) -> bool {
    (RESERVED_METADATA_START..RESERVED_METADATA_END).contains(&bytes)
        || bytes.first() == Some(&0xff)
}

pub(super) fn range_overlaps_metadata(begin: &[u8], end: &[u8]) -> bool {
    begin < RESERVED_METADATA_END && end > RESERVED_METADATA_START
}
