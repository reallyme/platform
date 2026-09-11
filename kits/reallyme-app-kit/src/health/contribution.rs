// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

/// App health contribution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppHealthStatus {
    /// App is ready from its own perspective.
    Ready,
    /// App is alive but should not receive traffic.
    NotReady,
    /// App health cannot be determined.
    Unknown,
}

/// Host-neutral app health contribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppHealthContribution {
    status: AppHealthStatus,
}

impl AppHealthContribution {
    /// Constructs a health contribution.
    pub const fn new(status: AppHealthStatus) -> Self {
        Self { status }
    }

    /// Constructs a ready contribution.
    pub const fn ready() -> Self {
        Self::new(AppHealthStatus::Ready)
    }

    /// Constructs a not-ready contribution.
    pub const fn not_ready() -> Self {
        Self::new(AppHealthStatus::NotReady)
    }

    /// Returns the app health contribution status.
    pub const fn status(self) -> AppHealthStatus {
        self.status
    }
}

/// Returns a ready app health contribution.
///
/// Many apps have no app-local readiness contribution beyond successful
/// startup. This helper keeps that default explicit and shared without giving
/// app-kit ownership of process readiness.
pub const fn ready_app_health_contribution() -> AppHealthContribution {
    AppHealthContribution::ready()
}
