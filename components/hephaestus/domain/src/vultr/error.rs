// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Vultr domain validation errors.

use thiserror::Error;

/// Vultr domain validation failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum VultrError {
    /// The value was empty after boundary normalization.
    #[error("vultr value is empty")]
    Empty,
    /// The value exceeded the fixed maximum length for its type.
    #[error("vultr value is too long")]
    TooLong,
    /// The value contained a character outside the allowlist for its type.
    #[error("vultr value contains an invalid character")]
    InvalidCharacter,
    /// The value was outside the accepted numeric range for its type.
    #[error("vultr value is outside the accepted range")]
    InvalidNumber,
    /// The value did not match a known Vultr instance status.
    #[error("vultr instance status is unknown")]
    UnknownInstanceStatus,
    /// The value did not match a known Vultr plan type.
    #[error("vultr plan type is unknown")]
    UnknownPlanType,
}
