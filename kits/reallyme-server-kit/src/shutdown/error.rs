// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

use crate::startup::TaskName;
use crate::task::{ShutdownTimeout, TaskExecutionErrorKind, TaskSetCapacity};

/// Signal-registration failures encountered during shutdown listener setup.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ShutdownError {
    /// The runtime could not install the requested signal listener.
    #[error("failed to install shutdown signal listener")]
    ListenerInstallFailed {
        /// The listener that failed to install.
        kind: ShutdownSignalKind,
    },
    /// The configured shutdown timeout is invalid.
    #[error("invalid shutdown timeout")]
    InvalidTimeout {
        /// The specific validation reason.
        reason: ShutdownValidationErrorReason,
    },
    /// A new background task was registered after shutdown had already begun.
    #[error("cannot register background task after shutdown has started")]
    ShutdownAlreadyRequested {
        /// The task that was rejected.
        task_name: TaskName,
    },
    /// A background task did not finish before the graceful shutdown deadline.
    #[error("background task did not finish before shutdown timeout")]
    TaskShutdownTimedOut {
        /// The task that exceeded the shutdown deadline.
        task_name: TaskName,
        /// The shutdown timeout that was enforced.
        timeout: ShutdownTimeout,
    },
    /// A background task could not be joined cleanly.
    #[error("background task could not be joined during shutdown")]
    TaskJoinFailed {
        /// The task that failed to join.
        task_name: TaskName,
        /// The typed join failure reason.
        reason: TaskJoinFailureReason,
    },
    /// A background task exited with a typed operational failure.
    #[error("background task exited with an operational failure")]
    TaskExitedWithError {
        /// The task that exited with an error.
        task_name: TaskName,
        /// The low-cardinality failure kind reported by the task.
        kind: TaskExecutionErrorKind,
    },
    /// A task registration would exceed the configured task-set capacity.
    #[error("background task set capacity would be exceeded")]
    TaskCapacityExceeded {
        /// The task that was rejected.
        task_name: TaskName,
        /// The configured task-set capacity.
        capacity: TaskSetCapacity,
    },
}

/// Enumerates the supported signal listeners so failures remain fully typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignalKind {
    /// `CTRL+C` listener registration.
    CtrlC,
    /// `SIGTERM` listener registration.
    Sigterm,
}

/// Validation failures for shutdown-specific configuration primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownValidationErrorReason {
    /// The value must be greater than zero.
    MustBeGreaterThanZero,
}

/// Typed join failures for spawned background tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskJoinFailureReason {
    /// The task was cancelled outside the coordinated timeout path.
    Cancelled,
    /// The task panicked while executing.
    Panicked,
    /// The runtime reported a join failure that was neither panic nor cancel.
    Unknown,
}
