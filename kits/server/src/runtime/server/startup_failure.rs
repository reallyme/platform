// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded cleanup after startup fails before the serving phase.

use crate::observability::{ErrorKind, log_error};
use crate::shutdown::ShutdownReason;
use crate::startup::ServerName;
use crate::task::{BackgroundTaskSet, ShutdownTimeout};

use super::super::app::RuntimeAppCleanup;
use super::super::cleanup::run_cleanup_hooks;
use super::super::error::ServerRuntimeError;

pub(super) async fn finish_failed_startup(
    error: ServerRuntimeError,
    tasks: &mut BackgroundTaskSet,
    cleanup_hooks: Vec<RuntimeAppCleanup>,
    shutdown_timeout: ShutdownTimeout,
    cleanup_timeout: ShutdownTimeout,
    server_name: &ServerName,
) -> ServerRuntimeError {
    // A later bind or registration may fail after earlier listeners and app
    // resources exist. Stop managed work before releasing app-owned resources.
    if tasks
        .shutdown(ShutdownReason::Unknown, shutdown_timeout)
        .await
        .is_err()
    {
        log_error(
            ErrorKind::Internal,
            "managed task drain failed after startup failure",
            None,
            None,
        );
    }
    if run_cleanup_hooks(cleanup_hooks, cleanup_timeout, server_name)
        .await
        .is_err()
    {
        log_error(
            ErrorKind::Internal,
            "app cleanup failed after startup failure",
            None,
            None,
        );
    }
    error
}
