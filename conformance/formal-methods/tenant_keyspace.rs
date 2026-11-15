// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

#[path = "../../kits/foundationdb/src/fdb/tenant/data_keyspace.rs"]
mod keyspace;

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
