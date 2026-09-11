// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! FoundationDB tuple encoding boundary.

use bytes::Bytes;

pub use foundationdb::tuple::{
    Element, Subspace, TuplePack, Versionstamp, pack, pack_with_versionstamp, unpack,
};

/// Encodes tuple values into a `Bytes` key buffer.
///
/// This avoids forcing downstream call sites to keep converting from `Vec<u8>`
/// when the surrounding key pipeline is already `Bytes`-oriented.
pub fn pack_bytes<T: TuplePack>(value: &T) -> Bytes {
    Bytes::from(pack(value))
}

#[cfg(test)]
mod tests {
    use super::{pack, pack_bytes};

    #[test]
    fn pack_bytes_matches_pack_output() {
        let value = ("tenant", 1_i64);
        let bytes_encoded = pack_bytes(&value);
        let vec_encoded = pack(&value);
        assert_eq!(bytes_encoded.as_ref(), vec_encoded.as_slice());
    }
}
