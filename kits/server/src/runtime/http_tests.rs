// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{HttpRateLimitScope, HttpRateLimitTierPolicy, HttpRateLimitTierPolicyErrorReason};

#[test]
fn rate_limit_tier_policy_rejects_non_admitting_or_wasteful_values() {
    assert_eq!(
        HttpRateLimitTierPolicy::new(0, 0, 1).map_err(|error| error.reason()),
        Err(HttpRateLimitTierPolicyErrorReason::ZeroBurstTokens)
    );
    assert_eq!(
        HttpRateLimitTierPolicy::new(0, 1, 0).map_err(|error| error.reason()),
        Err(HttpRateLimitTierPolicyErrorReason::ZeroMaxDistinctSources)
    );
    assert_eq!(
        HttpRateLimitTierPolicy::new(2, 1, 1).map_err(|error| error.reason()),
        Err(HttpRateLimitTierPolicyErrorReason::RefillExceedsBurst)
    );
}

#[test]
fn rate_limit_tier_policy_accepts_valid_boundaries() {
    let policy = HttpRateLimitTierPolicy::new(0, 1, 1)
        .expect("valid policy")
        .with_scope(HttpRateLimitScope::Shared);

    assert_eq!(policy.refill_tokens_per_second(), 0);
    assert_eq!(policy.burst_tokens(), 1);
    assert_eq!(policy.max_distinct_sources(), 1);
    assert_eq!(policy.scope(), HttpRateLimitScope::Shared);
}
