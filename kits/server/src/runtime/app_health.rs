// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::time::Duration;

use reallyme_app_kit::{AppHealthContributor, AppHealthStatus};
use tokio::time::{MissedTickBehavior, interval, timeout};

use crate::health::Readiness;
use crate::task::{ShutdownToken, TaskExecutionError};

// A health check is a probe, not a request handler. Bound the entire round so
// one stalled app cannot leave readiness dependent on a hung future.
const APP_HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(2);
const APP_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(5);

struct AppHealthFailureGuard(Readiness);

impl Drop for AppHealthFailureGuard {
    fn drop(&mut self) {
        // If an app callback panics or the monitor is cancelled, no previous
        // successful sample may continue to advertise readiness.
        self.0.set_app_health(false);
    }
}

pub(crate) async fn run_app_health_monitor(
    contributors: Vec<Arc<dyn AppHealthContributor>>,
    readiness: Readiness,
    mut shutdown: ShutdownToken,
) -> Result<(), TaskExecutionError> {
    let _failure_guard = AppHealthFailureGuard(readiness.clone());
    let mut ticks = interval(APP_HEALTH_CHECK_INTERVAL);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => return Ok(()),
            _ = ticks.tick() => {
                let healthy = timeout(
                    APP_HEALTH_CHECK_TIMEOUT,
                    check_app_health(contributors.as_slice()),
                )
                .await
                .unwrap_or(false);
                readiness.set_app_health(healthy);
            }
        }
    }
}

async fn check_app_health(contributors: &[Arc<dyn AppHealthContributor>]) -> bool {
    for contributor in contributors {
        match contributor.app_health().await {
            Ok(contribution) if contribution.status() == AppHealthStatus::Ready => {}
            Ok(_) | Err(_) => return false,
        }
    }
    true
}

#[cfg(test)]
#[path = "app_health_tests.rs"]
mod tests;
