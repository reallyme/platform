// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;

use super::{AppLifecycleError, AppLifecycleName};

/// Future returned by an app startup check.
pub type AppStartupCheckFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), AppLifecycleError>> + Send + 'a>>;

/// Host-neutral app startup check.
///
/// Hosts run startup checks before marking their app or process ready. Checks
/// should validate app-owned dependencies and configuration, not bind listeners
/// or perform host lifecycle work.
pub trait AppStartupCheck: Send + Sync {
    /// Returns the stable startup-check name.
    fn name(&self) -> &AppLifecycleName;

    /// Runs the startup check.
    fn run(&self) -> AppStartupCheckFuture<'_>;
}
