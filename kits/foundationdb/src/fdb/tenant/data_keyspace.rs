// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{convert::TryFrom, ops::Range};

pub(crate) const RESERVED_METADATA_START: &[u8] = b"__meta";
pub(crate) const RESERVED_METADATA_END: &[u8] = b"__metb";
const VERSIONSTAMP_BYTES: usize = 10;

pub(super) fn is_reserved_key(bytes: &[u8]) -> bool {
    (RESERVED_METADATA_START..RESERVED_METADATA_END).contains(&bytes)
        || bytes.first() == Some(&0xff)
}

pub(super) fn range_overlaps_metadata(begin: &[u8], end: &[u8]) -> bool {
    begin < RESERVED_METADATA_END && end > RESERVED_METADATA_START
}

pub(super) fn versionstamp_range(key_len: usize, offset: u32) -> Option<Range<usize>> {
    let offset = usize::try_from(offset).ok()?;
    let end = offset.checked_add(VERSIONSTAMP_BYTES)?;
    // Classifying system keys and metadata needs only the stable prefix.
    (offset >= RESERVED_METADATA_START.len() && end <= key_len).then_some(offset..end)
}
