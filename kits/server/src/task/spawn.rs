// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;

use tokio::task::JoinHandle;
use tracing::Instrument;

use super::error::TaskExecutionError;
use super::shutdown::ShutdownToken;
use crate::startup::TaskName;

pub(super) type ManagedTaskOutput = Result<(), TaskExecutionError>;

pub(super) struct ManagedBackgroundTask {
    pub(super) name: TaskName,
    pub(super) handle: JoinHandle<ManagedTaskOutput>,
}

pub(super) fn spawn_managed_background_task<F, Fut>(
    task_name: TaskName,
    task: F,
    shutdown: ShutdownToken,
) -> ManagedBackgroundTask
where
    F: FnOnce(ShutdownToken) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    spawn_managed_fallible_background_task(
        task_name,
        move |shutdown| async move {
            task(shutdown).await;
            Ok(())
        },
        shutdown,
    )
}

pub(super) fn spawn_managed_fallible_background_task<F, Fut>(
    task_name: TaskName,
    task: F,
    shutdown: ShutdownToken,
) -> ManagedBackgroundTask
where
    F: FnOnce(ShutdownToken) -> Fut + Send + 'static,
    Fut: Future<Output = ManagedTaskOutput> + Send + 'static,
{
    let span = tracing::info_span!("background_task", task.name = task_name.as_str());
    let task_name_for_task = task_name.clone();
    let handle = tokio::spawn(
        async move {
            let result = task(shutdown).await;

            if let Err(error) = result {
                tracing::error!(
                    task.name = task_name_for_task.as_str(),
                    error.kind = error.kind().as_str(),
                    "background task exited with error"
                );
            }

            result
        }
        .instrument(span),
    );

    ManagedBackgroundTask {
        name: task_name,
        handle,
    }
}
