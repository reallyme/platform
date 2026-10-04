// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::{RuntimeStartupCheck, StartupCheckTimeout, StartupCheckTimeoutErrorReason};
use crate::startup::TaskName;
use crate::task::{TaskExecutionError, TaskExecutionErrorKind};

#[tokio::test]
async fn startup_check_reports_typed_failure_kind() {
    let check = RuntimeStartupCheck::new(
        TaskName::new("dependency-check").expect("valid fixture task name"),
        || async {
            Err(TaskExecutionError::new(
                TaskExecutionErrorKind::DependencyUnavailable,
            ))
        },
    );

    assert_eq!(
        check.run().await,
        Err(TaskExecutionErrorKind::DependencyUnavailable)
    );
}

#[tokio::test]
async fn startup_check_times_out_instead_of_blocking_readiness() {
    let timeout = StartupCheckTimeout::new(Duration::from_millis(10))
        .expect("short test deadline should be valid");
    let check = RuntimeStartupCheck::new(
        TaskName::new("pending-check").expect("valid fixture task name"),
        || async { std::future::pending::<Result<(), TaskExecutionError>>().await },
    )
    .with_timeout(timeout);

    assert_eq!(check.run().await, Err(TaskExecutionErrorKind::TimedOut));
}

#[test]
fn startup_check_timeout_rejects_invalid_bounds() {
    assert!(matches!(
        StartupCheckTimeout::new(Duration::ZERO),
        Err(error) if error.reason() == StartupCheckTimeoutErrorReason::MustBeGreaterThanZero
    ));
    assert!(matches!(
        StartupCheckTimeout::new(Duration::from_secs(301)),
        Err(error) if error.reason() == StartupCheckTimeoutErrorReason::ExceedsMaximum
    ));
}
