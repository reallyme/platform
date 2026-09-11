// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed key label validation errors.

use thiserror::Error;

/// Stable key-label validation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KeyLabelError {
    /// Key labels must contain at least one byte.
    #[error("key label is empty")]
    Empty,
    /// Key labels exceeded the bounded length policy.
    #[error("key label is too long")]
    TooLong,
    /// Key labels contained a byte outside the allowlist.
    #[error("key label contains an invalid byte")]
    InvalidByte,
}
