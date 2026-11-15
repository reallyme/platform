// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

pub(crate) fn deadline_allows_work(elapsed: Duration, timeout: Option<Duration>) -> bool {
    timeout.is_none_or(|timeout| elapsed < timeout)
}

pub(super) fn retry_budget_allows(
    attempts: u32,
    limit: Option<u32>,
    elapsed: Duration,
    timeout: Option<Duration>,
) -> bool {
    limit.is_none_or(|limit| attempts < limit) && deadline_allows_work(elapsed, timeout)
}

pub(crate) const fn retry_permitted(
    error_is_retryable: bool,
    error_is_maybe_committed: bool,
    operation_is_idempotent: bool,
    budget_allows_retry: bool,
) -> bool {
    error_is_retryable
        && (operation_is_idempotent || !error_is_maybe_committed)
        && budget_allows_retry
}
