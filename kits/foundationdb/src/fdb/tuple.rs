// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
#[path = "tuple_tests.rs"]
mod tests;
