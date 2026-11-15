// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

#[path = "../../kits/foundationdb/src/fdb/tenant/transact_budget.rs"]
mod budget;

use std::time::Duration;

#[kani::proof]
fn an_expired_or_exhausted_budget_and_ambiguous_writes_bar_retry() {
    let attempts: u32 = kani::any();
    let limit: Option<u32> = kani::any();
    let elapsed_secs: u64 = kani::any();
    let elapsed_nanos: u32 = kani::any();
    let timeout_secs: u64 = kani::any();
    let timeout_nanos: u32 = kani::any();
    kani::assume(elapsed_nanos < 1_000_000_000 && timeout_nanos < 1_000_000_000);
    let bounded: bool = kani::any();
    let idempotent: bool = kani::any();
    let maybe_committed: bool = kani::any();
    let retryable: bool = kani::any();
    let elapsed = Duration::new(elapsed_secs, elapsed_nanos);
    let timeout = bounded.then(|| Duration::new(timeout_secs, timeout_nanos));
    let time_remaining = !bounded || (elapsed_secs, elapsed_nanos) < (timeout_secs, timeout_nanos);
    let attempts_remaining = limit.is_none_or(|limit| attempts < limit);
    assert_eq!(
        budget::deadline_allows_work(elapsed, timeout),
        time_remaining
    );
    let allowed = budget::retry_budget_allows(attempts, limit, elapsed, timeout);
    assert_eq!(allowed, time_remaining && attempts_remaining);
    let retry = budget::retry_permitted(retryable, maybe_committed, idempotent, allowed);
    if retry {
        assert!(retryable && time_remaining && attempts_remaining);
        assert!(idempotent || !maybe_committed);
    }
    if retryable && time_remaining && attempts_remaining && !maybe_committed {
        assert!(retry);
    }
}
