// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral app lifecycle contracts.

mod background;
mod cleanup;
mod descriptor;
mod error;
mod name;
mod shutdown;
mod startup;

pub use background::{AppBackgroundTask, AppBackgroundTaskFuture};
pub use cleanup::{AppCleanupFuture, AppCleanupHook};
pub use descriptor::{
    AppBackgroundTaskDescriptor, AppCleanupHookDescriptor, AppStartupCheckDescriptor,
};
pub use error::{AppLifecycleError, AppLifecycleErrorKind};
pub use name::AppLifecycleName;
pub use shutdown::AppTaskShutdown;
pub use startup::{AppStartupCheck, AppStartupCheckFuture};
