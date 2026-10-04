// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Process signal ownership and explicit second-signal escalation.

use tokio::sync::mpsc;

use crate::shutdown::{ShutdownError, install_shutdown_signal_listener};
use crate::startup::TaskName;
use crate::task::{BackgroundTaskSet, TaskExecutionError, TaskExecutionErrorKind};

use super::{ServerRuntime, ServerRuntimeError};

pub(super) async fn run_with_os_signals(runtime: ServerRuntime) -> Result<(), ServerRuntimeError> {
    let mut signals = install_shutdown_signal_listener()
        .map_err(|source| ServerRuntimeError::Shutdown { source })?;
    let (sender, mut receiver) = mpsc::channel(1);
    let mut signal_tasks = BackgroundTaskSet::new();
    let task_name = TaskName::new("shutdown-signal-listener")?;
    signal_tasks
        .spawn_fallible(task_name, move |mut shutdown| async move {
            let first = tokio::select! {
                result = signals.wait_next() => result,
                _reason = shutdown.cancelled() => return Ok(()),
            }
            .map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))?;
            if sender.send(first).await.is_err() {
                return Ok(());
            }

            tokio::select! {
                _reason = shutdown.cancelled() => Ok(()),
                second = signals.wait_next() => {
                    if second.is_ok() {
                        // A second explicit process signal abandons a drain
                        // that may be stuck on a broken external dependency.
                        std::process::exit(1);
                    }
                    Err(TaskExecutionError::new(TaskExecutionErrorKind::Internal))
                }
            }
        })
        .map_err(|source| ServerRuntimeError::TaskRegistration { source })?;

    let result = runtime
        .run_until_shutdown(async move {
            receiver
                .recv()
                .await
                .ok_or(ShutdownError::SignalChannelClosed)
        })
        .await;
    drop(signal_tasks);
    result
}
