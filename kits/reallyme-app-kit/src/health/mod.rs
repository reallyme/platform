// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral app health contribution contracts.

mod contribution;
mod contributor;

pub use contribution::{AppHealthContribution, AppHealthStatus, ready_app_health_contribution};
pub use contributor::{AppHealthContributor, AppHealthFuture};
