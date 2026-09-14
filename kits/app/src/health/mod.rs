// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral app health contribution contracts.

mod contribution;
mod contributor;

pub use contribution::{AppHealthContribution, AppHealthStatus, ready_app_health_contribution};
pub use contributor::{AppHealthContributor, AppHealthFuture};
