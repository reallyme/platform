// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

use super::AppKitField;

/// Typed app-kit validation error reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKitErrorReason {
    /// The value was empty.
    Empty,
    /// The value was too long.
    TooLong,
    /// The value had an invalid boundary.
    InvalidBoundary,
    /// The value contained invalid characters.
    InvalidCharacter,
}

/// App-kit validation error.
///
/// This error intentionally carries only low-cardinality enums. It never
/// includes raw app config, secrets, URLs, tokens, or arbitrary parser text.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("app-kit validation failed")]
pub struct AppKitError {
    field: AppKitField,
    reason: AppKitErrorReason,
}

impl AppKitError {
    /// Constructs an app-kit validation error.
    pub const fn new(field: AppKitField, reason: AppKitErrorReason) -> Self {
        Self { field, reason }
    }

    /// Returns the field that failed validation.
    pub const fn field(self) -> AppKitField {
        self.field
    }

    /// Returns the typed validation reason.
    pub const fn reason(self) -> AppKitErrorReason {
        self.reason
    }
}
