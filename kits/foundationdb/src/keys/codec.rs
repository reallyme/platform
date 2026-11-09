// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Key codec traits for repository-owned encoders.

use bytes::Bytes;

/// Encodes a typed key into FoundationDB tuple bytes.
pub trait KeyEncoder {
    /// Returns the encoded key.
    fn encode_key(&self) -> Bytes;
}
