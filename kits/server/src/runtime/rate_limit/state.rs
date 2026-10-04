// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded source buckets with indexed least-recent-activity eviction.

use std::collections::{BTreeSet, HashMap};
use std::time::Instant;

use super::{
    HttpRateLimitTierPolicy, RATE_LIMIT_BUCKET_IDLE_TTL_SECS, SourceRateBucket, refill_bucket,
};
use crate::http::HttpRateLimitTierName;

type TierBuckets = HashMap<u64, SourceRateBucket>;
type GlobalAgeKey = (Instant, HttpRateLimitTierName, u64);
// Bound work under source churn while allowing a refilled bucket behind a
// recently active debtor to make room for a new client.
const MAX_EVICTION_CANDIDATES: usize = 32;

#[derive(Debug, Default)]
pub(super) struct RateLimitBucketState {
    pub(super) by_tier: HashMap<HttpRateLimitTierName, TierBuckets>,
    tier_age: HashMap<HttpRateLimitTierName, BTreeSet<(Instant, u64)>>,
    global_age: BTreeSet<GlobalAgeKey>,
    // At most one shared bucket per configured tier serves newcomers when
    // every retained source still has rate-limit debt.
    pub(super) overflow: HashMap<HttpRateLimitTierName, SourceRateBucket>,
}

impl RateLimitBucketState {
    pub(super) fn len(&self) -> usize {
        self.global_age.len()
    }

    pub(super) fn tier_len(&self, tier: &HttpRateLimitTierName) -> usize {
        self.by_tier.get(tier).map_or(0, HashMap::len)
    }

    pub(super) fn contains(&self, tier: &HttpRateLimitTierName, source: u64) -> bool {
        self.by_tier
            .get(tier)
            .is_some_and(|entries| entries.contains_key(&source))
    }

    pub(super) fn bucket_mut(
        &mut self,
        tier: &HttpRateLimitTierName,
        source: u64,
    ) -> Option<&mut SourceRateBucket> {
        self.by_tier.get_mut(tier)?.get_mut(&source)
    }

    pub(super) fn insert(
        &mut self,
        tier: HttpRateLimitTierName,
        source: u64,
        bucket: SourceRateBucket,
    ) {
        let active_at = bucket.last_activity_at;
        self.by_tier
            .entry(tier.clone())
            .or_default()
            .insert(source, bucket);
        self.tier_age
            .entry(tier.clone())
            .or_default()
            .insert((active_at, source));
        self.global_age.insert((active_at, tier, source));
    }

    pub(super) fn record_activity(
        &mut self,
        tier: &HttpRateLimitTierName,
        source: u64,
        now: Instant,
    ) {
        let Some(bucket) = self.bucket_mut(tier, source) else {
            return;
        };
        let previous = bucket.last_activity_at;
        bucket.last_activity_at = now;
        if let Some(age) = self.tier_age.get_mut(tier) {
            age.remove(&(previous, source));
            age.insert((now, source));
        }
        self.global_age.remove(&(previous, tier.clone(), source));
        self.global_age.insert((now, tier.clone(), source));
    }

    pub(super) fn evict_refilled_tier_bucket(
        &mut self,
        tier: &HttpRateLimitTierName,
        policy: HttpRateLimitTierPolicy,
        now: Instant,
    ) -> bool {
        let candidate = self.tier_age.get(tier).and_then(|age| {
            age.iter()
                .take(MAX_EVICTION_CANDIDATES)
                .find(|(_, source)| {
                    self.by_tier
                        .get(tier)
                        .and_then(|entries| entries.get(source))
                        .is_some_and(|bucket| bucket_has_refilled(bucket, policy, now))
                })
                .map(|(_, source)| *source)
        });
        candidate.is_some_and(|source| self.remove(tier, source))
    }

    pub(super) fn evict_refilled_global_bucket(
        &mut self,
        policies: &[(HttpRateLimitTierName, HttpRateLimitTierPolicy)],
        now: Instant,
    ) -> bool {
        let candidate = self
            .global_age
            .iter()
            .take(MAX_EVICTION_CANDIDATES)
            .find(|(_, tier, source)| {
                self.tier_len(tier) > 1
                    && policies
                        .iter()
                        .find_map(|(name, policy)| (name == tier).then_some(*policy))
                        .is_some_and(|policy| {
                            self.by_tier
                                .get(tier)
                                .and_then(|entries| entries.get(source))
                                .is_some_and(|bucket| bucket_has_refilled(bucket, policy, now))
                        })
            })
            .map(|(_, tier, source)| (tier.clone(), *source));
        candidate.is_some_and(|(tier, source)| self.remove(&tier, source))
    }

    pub(super) fn overflow_bucket(
        &mut self,
        tier: &HttpRateLimitTierName,
        now: Instant,
    ) -> &mut SourceRateBucket {
        self.overflow
            .entry(tier.clone())
            // One initial token keeps a newly arrived client serviceable when
            // the table first fills, without minting a burst per identity.
            .or_insert_with(|| SourceRateBucket::fresh(now, 1))
    }

    pub(super) fn prune_stale(
        &mut self,
        policies: &[(HttpRateLimitTierName, HttpRateLimitTierPolicy)],
        now: Instant,
    ) {
        let stale: Vec<_> = self
            .by_tier
            .iter()
            .flat_map(|(tier, entries)| {
                let policy = policies
                    .iter()
                    .find_map(|(name, policy)| (name == tier).then_some(*policy));
                entries
                    .iter()
                    .filter(move |(_, bucket)| {
                        now.saturating_duration_since(bucket.last_activity_at)
                            .as_secs()
                            >= RATE_LIMIT_BUCKET_IDLE_TTL_SECS
                            && policy.is_some_and(|policy| bucket_has_refilled(bucket, policy, now))
                    })
                    .map(|(source, _)| (tier.clone(), *source))
            })
            .collect();
        for (tier, source) in stale {
            self.remove(&tier, source);
        }
        self.overflow.retain(|tier, bucket| {
            let stale = now
                .saturating_duration_since(bucket.last_activity_at)
                .as_secs()
                >= RATE_LIMIT_BUCKET_IDLE_TTL_SECS;
            let refilled = policies
                .iter()
                .find_map(|(name, policy)| (name == tier).then_some(*policy))
                .is_some_and(|policy| bucket_has_refilled(bucket, policy, now));
            !stale || !refilled
        });
    }

    fn remove(&mut self, tier: &HttpRateLimitTierName, source: u64) -> bool {
        let Some(entries) = self.by_tier.get_mut(tier) else {
            return false;
        };
        let Some(bucket) = entries.remove(&source) else {
            return false;
        };
        self.global_age
            .remove(&(bucket.last_activity_at, tier.clone(), source));
        if let Some(age) = self.tier_age.get_mut(tier) {
            age.remove(&(bucket.last_activity_at, source));
        }
        if entries.is_empty() {
            self.by_tier.remove(tier);
            self.tier_age.remove(tier);
        }
        true
    }
}

fn bucket_has_refilled(
    bucket: &SourceRateBucket,
    policy: HttpRateLimitTierPolicy,
    now: Instant,
) -> bool {
    let mut current = *bucket;
    refill_bucket(&mut current, policy, now);
    current.tokens == policy.burst_tokens()
}

#[cfg(test)]
#[path = "state_tests.rs"]
pub(super) mod tests;
