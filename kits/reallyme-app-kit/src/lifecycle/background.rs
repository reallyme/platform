// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;

use super::{AppLifecycleError, AppLifecycleName, AppTaskShutdown};

/// Future returned by an app background task.
pub type AppBackgroundTaskFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), AppLifecycleError>> + Send + 'a>>;

/// Host-neutral app background task contract.
///
/// The host owns task supervision and cancellation. Apps provide bounded,
/// cancellation-aware work through this trait.
pub trait AppBackgroundTask: Send + Sync {
    /// Returns the stable task name.
    fn name(&self) -> &AppLifecycleName;

    /// Runs the background task using a host-provided shutdown observer.
    fn run<'a>(&'a self, shutdown: &'a dyn AppTaskShutdown) -> AppBackgroundTaskFuture<'a>;
}
