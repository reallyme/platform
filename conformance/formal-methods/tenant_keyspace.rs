// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

#[path = "../../kits/foundationdb/src/fdb/tenant/data_keyspace.rs"]
mod keyspace;

use std::convert::TryFrom;

#[kani::proof]
#[kani::unwind(10)]
fn recognizes_the_metadata_prefix_and_system_keyspace() {
    let storage: [u8; 8] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= storage.len());
    let key = &storage[..len];
    assert_eq!(
        keyspace::is_reserved_key(key),
        key.starts_with(b"__meta") || key.starts_with(b"\xff"),
    );
}

#[kani::proof]
#[kani::unwind(10)]
fn an_admitted_range_excludes_any_reserved_key_in_its_interval() {
    let bounds: [([u8; 8], usize); 3] = kani::any();
    let [begin, end, candidate] = bounds.each_ref().map(|(bytes, len)| {
        kani::assume(*len <= bytes.len());
        &bytes[..*len]
    });
    kani::assume(!begin.is_empty() && !end.is_empty() && begin < end);
    kani::assume(!keyspace::is_reserved_key(begin) && !keyspace::is_reserved_key(end));
    kani::assume(!keyspace::range_overlaps_metadata(begin, end));
    kani::assume(begin <= candidate && candidate < end);
    assert!(!keyspace::is_reserved_key(candidate));
}

#[kani::proof]
fn a_versionstamp_replaces_exactly_ten_bytes_after_the_reserved_prefix() {
    let key_len: usize = kani::any();
    let offset: u32 = kani::any();
    let expected = usize::try_from(offset).ok().is_some_and(|offset| {
        offset >= b"__meta".len() && key_len.checked_sub(offset).is_some_and(|left| left >= 10)
    });
    let range = keyspace::versionstamp_range(key_len, offset);
    assert_eq!(range.is_some(), expected);
    if let Some(range) = range {
        assert_eq!(u32::try_from(range.start), Ok(offset));
        assert_eq!(range.end - range.start, 10);
        assert!(range.start >= 6 && range.end <= key_len);
    }
}
