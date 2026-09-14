// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{pack, pack_bytes};

#[test]
fn pack_bytes_matches_pack_output() {
    let value = ("tenant", 1_i64);
    let bytes_encoded = pack_bytes(&value);
    let vec_encoded = pack(&value);
    assert_eq!(bytes_encoded.as_ref(), vec_encoded.as_slice());
}
