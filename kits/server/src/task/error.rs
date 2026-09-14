// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Low-cardinality failure kinds for long-lived background tasks.
///
/// Apps should map richer internal failures into one of these stable kinds
/// before returning from a managed task. This keeps task reporting safe for
/// logs, metrics, and shutdown surfaces without leaking implementation detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskExecutionErrorKind {
    /// A required dependency became unavailable.
    DependencyUnavailable,
    /// The task encountered invalid or unsupported runtime configuration.
    InvalidConfiguration,
    /// The task failed for an internal reason that should be investigated.
    Internal,
}

impl TaskExecutionErrorKind {
    /// Returns the stable structured-log value for this failure kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DependencyUnavailable => "dependency_unavailable",
            Self::InvalidConfiguration => "invalid_configuration",
            Self::Internal => "internal",
        }
    }
}

/// Typed background-task execution failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("background task failed")]
pub struct TaskExecutionError {
    kind: TaskExecutionErrorKind,
}

impl TaskExecutionError {
    /// Creates a typed background-task execution failure.
    pub const fn new(kind: TaskExecutionErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the low-cardinality failure kind.
    pub const fn kind(self) -> TaskExecutionErrorKind {
        self.kind
    }
}

/// Validation failures for bounded task-channel helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskChannelValidationErrorReason {
    /// The value must be greater than zero.
    MustBeGreaterThanZero,
}

/// Typed validation error for bounded task-channel helpers.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("invalid bounded task channel capacity")]
pub struct TaskChannelError {
    reason: TaskChannelValidationErrorReason,
}

impl TaskChannelError {
    /// Creates a typed channel-capacity validation failure.
    pub const fn new(reason: TaskChannelValidationErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the validation failure reason.
    pub const fn reason(self) -> TaskChannelValidationErrorReason {
        self.reason
    }
}

/// Validation failures for background-task set sizing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSetValidationErrorReason {
    /// The value must be greater than zero.
    MustBeGreaterThanZero,
    /// The value must be less than or equal to the configured maximum.
    MustBeLessThanOrEqualToMaximum,
}

/// Typed validation error for background-task set capacity.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("invalid background task set capacity")]
pub struct TaskSetError {
    reason: TaskSetValidationErrorReason,
}

impl TaskSetError {
    /// Creates a typed task-set capacity validation failure.
    pub const fn new(reason: TaskSetValidationErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the validation failure reason.
    pub const fn reason(self) -> TaskSetValidationErrorReason {
        self.reason
    }
}
