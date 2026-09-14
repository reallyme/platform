// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Low-cardinality app lifecycle failure kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLifecycleErrorKind {
    /// App configuration was invalid or incomplete.
    Configuration,
    /// A dependency required by the app was unavailable.
    DependencyUnavailable,
    /// Startup, cleanup, or task execution timed out.
    Timeout,
    /// App behavior is not implemented in the current phase.
    NotImplemented,
    /// The host requested cancellation or shutdown.
    Cancelled,
}

/// Typed app lifecycle failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("app lifecycle operation failed")]
pub struct AppLifecycleError {
    kind: AppLifecycleErrorKind,
}

impl AppLifecycleError {
    /// Constructs a lifecycle failure.
    pub const fn new(kind: AppLifecycleErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the low-cardinality failure kind.
    pub const fn kind(self) -> AppLifecycleErrorKind {
        self.kind
    }
}
