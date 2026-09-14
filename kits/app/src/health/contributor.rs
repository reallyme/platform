// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;

use super::AppHealthContribution;
use crate::lifecycle::AppLifecycleError;

/// Future returned by an app health contributor.
pub type AppHealthFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AppHealthContribution, AppLifecycleError>> + Send + 'a>>;

/// Host-neutral app health contributor.
pub trait AppHealthContributor: Send + Sync {
    /// Returns this app's health contribution.
    fn app_health(&self) -> AppHealthFuture<'_>;
}
