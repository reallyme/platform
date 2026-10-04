// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use reallyme_app_kit::{
    AppHealthContribution, AppHealthContributor, AppHealthFuture, AppHealthStatus,
    AppLifecycleError, AppLifecycleErrorKind,
};

use super::{check_app_health, run_app_health_monitor};
use crate::health::{Readiness, ReadinessState};
use crate::shutdown::ShutdownReason;
use crate::task::ShutdownController;

struct TestHealthContributor {
    state: AtomicU8,
}

impl TestHealthContributor {
    fn new(state: u8) -> Self {
        Self {
            state: AtomicU8::new(state),
        }
    }
}

impl AppHealthContributor for TestHealthContributor {
    fn app_health(&self) -> AppHealthFuture<'_> {
        Box::pin(async move {
            match self.state.load(Ordering::SeqCst) {
                0 => Ok(AppHealthContribution::ready()),
                1 => Ok(AppHealthContribution::not_ready()),
                2 => Ok(AppHealthContribution::new(AppHealthStatus::Unknown)),
                _ => Err(AppLifecycleError::new(
                    AppLifecycleErrorKind::DependencyUnavailable,
                )),
            }
        })
    }
}

#[tokio::test]
async fn contributor_statuses_fail_closed() {
    let contributor = Arc::new(TestHealthContributor::new(0));
    let contributors: Vec<Arc<dyn AppHealthContributor>> = vec![contributor.clone()];
    assert!(check_app_health(&contributors).await);

    for state in [1, 2, 3] {
        contributor.state.store(state, Ordering::SeqCst);
        assert!(!check_app_health(&contributors).await);
    }
}

#[tokio::test]
async fn monitor_samples_health_and_stops_on_shutdown() {
    let contributor: Arc<dyn AppHealthContributor> = Arc::new(TestHealthContributor::new(0));
    let readiness = Readiness::new();
    readiness.set_app_health(false);
    readiness.mark_ready();
    let controller = ShutdownController::new();
    let monitor = run_app_health_monitor(vec![contributor], readiness.clone(), controller.token());
    tokio::pin!(monitor);

    tokio::select! {
        result = &mut monitor => panic!("health monitor stopped early: {result:?}"),
        _ = tokio::time::sleep(Duration::from_millis(20)) => {}
    }
    assert_eq!(readiness.state(), ReadinessState::Ready);

    assert!(controller.begin_shutdown(ShutdownReason::Unknown));
    let result = tokio::time::timeout(Duration::from_secs(1), monitor)
        .await
        .expect("monitor should stop after shutdown");
    assert!(result.is_ok());
}
