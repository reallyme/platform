// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;

use super::{AppLifecycleError, AppLifecycleName};

/// Future returned by an app cleanup hook.
pub type AppCleanupFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), AppLifecycleError>> + Send + 'a>>;

/// Host-neutral app cleanup hook.
///
/// Hosts run cleanup during shutdown according to host-specific ordering and
/// timeout policy. Hooks are for app-owned resources only.
pub trait AppCleanupHook: Send + Sync {
    /// Returns the stable cleanup hook name.
    fn name(&self) -> &AppLifecycleName;

    /// Runs app cleanup.
    fn cleanup(&self) -> AppCleanupFuture<'_>;
}
