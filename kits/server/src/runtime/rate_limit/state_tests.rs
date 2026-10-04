// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::{BTreeSet, HashMap};

use super::RateLimitBucketState;

pub(in crate::runtime::rate_limit) fn indexes_are_consistent(state: &RateLimitBucketState) -> bool {
    let mapped = state.by_tier.values().map(HashMap::len).sum::<usize>();
    let tier_indexed = state.tier_age.values().map(BTreeSet::len).sum::<usize>();
    mapped == state.global_age.len()
        && mapped == tier_indexed
        && state.by_tier.iter().all(|(tier, entries)| {
            entries.iter().all(|(source, bucket)| {
                state
                    .global_age
                    .contains(&(bucket.last_admitted_at, tier.clone(), *source))
                    && state
                        .tier_age
                        .get(tier)
                        .is_some_and(|age| age.contains(&(bucket.last_admitted_at, *source)))
            })
        })
}
