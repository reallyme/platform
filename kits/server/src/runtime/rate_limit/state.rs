// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded source buckets with indexed oldest-admission eviction.

use std::collections::{BTreeSet, HashMap};
use std::time::Instant;

use super::{RATE_LIMIT_BUCKET_IDLE_TTL_SECS, SourceRateBucket};
use crate::http::HttpRateLimitTierName;

type TierBuckets = HashMap<u64, SourceRateBucket>;
type GlobalAgeKey = (Instant, HttpRateLimitTierName, u64);

#[derive(Debug, Default)]
pub(super) struct RateLimitBucketState {
    pub(super) by_tier: HashMap<HttpRateLimitTierName, TierBuckets>,
    tier_age: HashMap<HttpRateLimitTierName, BTreeSet<(Instant, u64)>>,
    global_age: BTreeSet<GlobalAgeKey>,
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
        let admitted_at = bucket.last_admitted_at;
        self.by_tier
            .entry(tier.clone())
            .or_default()
            .insert(source, bucket);
        self.tier_age
            .entry(tier.clone())
            .or_default()
            .insert((admitted_at, source));
        self.global_age.insert((admitted_at, tier, source));
    }

    pub(super) fn record_admission(
        &mut self,
        tier: &HttpRateLimitTierName,
        source: u64,
        now: Instant,
    ) {
        let Some(bucket) = self.bucket_mut(tier, source) else {
            return;
        };
        let previous = bucket.last_admitted_at;
        bucket.last_admitted_at = now;
        if let Some(age) = self.tier_age.get_mut(tier) {
            age.remove(&(previous, source));
            age.insert((now, source));
        }
        self.global_age.remove(&(previous, tier.clone(), source));
        self.global_age.insert((now, tier.clone(), source));
    }

    pub(super) fn evict_tier(&mut self, tier: &HttpRateLimitTierName) -> bool {
        let oldest = self
            .tier_age
            .get(tier)
            .and_then(|age| age.first().map(|(_, source)| *source));
        oldest.is_some_and(|source| self.remove(tier, source))
    }

    pub(super) fn evict_global(&mut self, preferred_tier: &HttpRateLimitTierName) -> bool {
        let oldest = self
            .global_age
            .iter()
            .find(|(_, tier, _)| self.tier_len(tier) > 1)
            .map(|(_, tier, source)| (tier.clone(), *source));
        if let Some((tier, source)) = oldest {
            return self.remove(&tier, source);
        }
        // Reserve one source per tier when possible; rotate the preferred
        // tier's sole source instead of evicting another tier entirely.
        self.evict_tier(preferred_tier)
    }

    pub(super) fn prune_stale(&mut self, now: Instant) {
        let stale: Vec<_> = self
            .by_tier
            .iter()
            .flat_map(|(tier, entries)| {
                entries
                    .iter()
                    .filter(|(_, bucket)| {
                        now.saturating_duration_since(bucket.last_activity_at)
                            .as_secs()
                            >= RATE_LIMIT_BUCKET_IDLE_TTL_SECS
                    })
                    .map(|(source, _)| (tier.clone(), *source))
            })
            .collect();
        for (tier, source) in stale {
            self.remove(&tier, source);
        }
    }

    #[cfg(test)]
    pub(super) fn indexes_are_consistent(&self) -> bool {
        let mapped = self.by_tier.values().map(HashMap::len).sum::<usize>();
        let tier_indexed = self.tier_age.values().map(BTreeSet::len).sum::<usize>();
        mapped == self.global_age.len()
            && mapped == tier_indexed
            && self.by_tier.iter().all(|(tier, entries)| {
                entries.iter().all(|(source, bucket)| {
                    self.global_age
                        .contains(&(bucket.last_admitted_at, tier.clone(), *source))
                        && self
                            .tier_age
                            .get(tier)
                            .is_some_and(|age| age.contains(&(bucket.last_admitted_at, *source)))
                })
            })
    }

    fn remove(&mut self, tier: &HttpRateLimitTierName, source: u64) -> bool {
        let Some(entries) = self.by_tier.get_mut(tier) else {
            return false;
        };
        let Some(bucket) = entries.remove(&source) else {
            return false;
        };
        self.global_age
            .remove(&(bucket.last_admitted_at, tier.clone(), source));
        if let Some(age) = self.tier_age.get_mut(tier) {
            age.remove(&(bucket.last_admitted_at, source));
        }
        if entries.is_empty() {
            self.by_tier.remove(tier);
            self.tier_age.remove(tier);
        }
        true
    }
}
