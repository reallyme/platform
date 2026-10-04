// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Generic host-neutral app port error.
///
/// Provider-specific failures belong to the app or adapter that owns the
/// provider. This boundary exposes only stable, low-cardinality classes.
#[non_exhaustive]
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AppPortError {
    /// The port was intentionally left unconfigured.
    #[error("app port is unconfigured")]
    Unconfigured,
    /// The port operation timed out before completion.
    #[error("app port operation timed out")]
    Timeout,
    /// The remote downstream port was unavailable.
    #[error("app downstream port unavailable")]
    Unavailable,
    /// The downstream response violated the expected contract.
    #[error("app port protocol violation")]
    ProtocolViolation,
    /// The requested downstream operation is not implemented.
    #[error("app port operation not implemented")]
    NotImplemented,
}

impl AppPortError {
    /// Returns the low-cardinality metric label for this port error.
    pub const fn as_metric_label(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::Timeout => "timeout",
            Self::Unavailable => "unavailable",
            Self::ProtocolViolation => "protocol_violation",
            Self::NotImplemented => "not_implemented",
        }
    }
}

/// Backward-compatible alias for call sites that were binding a separate error-kind
/// type. Prefer [`AppPortError`] with [`AppPortError::as_metric_label()`].
pub type AppPortErrorKind = AppPortError;

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
