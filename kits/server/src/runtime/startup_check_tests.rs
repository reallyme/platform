// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::RuntimeStartupCheck;
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
