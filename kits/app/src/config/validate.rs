// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Trait for typed app-owned configuration.
///
/// App configs should implement validation in the app crate because only the
/// app knows which fields are production-critical. Validation consumes `self`
/// so callers cannot accidentally continue with an unchecked mutable config
/// object.
pub trait AppConfig: Sized {
    /// Typed validation error.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Validates the app config and returns the immutable validated value.
    fn validate(self) -> Result<Self, Self::Error>;
}
