// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};
use std::net::{IpAddr, Ipv6Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LockResult, Mutex, MutexGuard};
use std::time::Instant;

use tokio::time::{Duration, MissedTickBehavior, interval};

use crate::http::HttpRateLimitTierName;
#[cfg(feature = "metrics")]
use crate::observability::{
    record_rate_limit_buckets_live, record_rate_limit_mutex_poisoned,
    record_rate_limit_overflow_decision,
};
use crate::runtime::{HttpRateLimitScope, HttpRateLimitTierPolicy};
use crate::task::{ShutdownToken, TaskExecutionError};

#[path = "rate_limit/state.rs"]
mod state;
use state::RateLimitBucketState;

/// Maximum number of retained per-source buckets per registry.
///
/// A full tier may also hold a fixed number of newcomer shards, bounded by
/// the number of configured tiers rather than caller-controlled identities.
pub const RATE_LIMIT_MAX_LIVE_BUCKETS: usize = 25_000;
const OVERFLOW_SHARD_LIMIT: u32 = 8;
// Reserve a bounded, separate allowance for clients arriving while retained
// sources still owe debt. Churn can spend at most this aggregate allowance;
// it cannot obtain a fresh bucket for each forged source address.
const OVERFLOW_BUDGET_MULTIPLIER: u32 = 2;

/// Idle bucket pruning window for the periodic sweep.
pub const RATE_LIMIT_BUCKET_IDLE_TTL_SECS: u64 = 900;

/// Periodic sweep cadence used by runtime-managed background tasks.
pub const RATE_LIMIT_BUCKET_SWEEP_INTERVAL_SECS: u64 = 60;

static RATE_LIMIT_BUCKET_MUTEX_POISON_WARNED: AtomicBool = AtomicBool::new(false);

/// Low-cost source identity used to compute per-source bucket keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitSourceIdentity {
    /// Normalized forwarded IP address.
    ForwardedIp(IpAddr),
    /// Direct peer socket IP address.
    PeerIp(IpAddr),
    /// No recognized source identity.
    Anonymous,
}

/// In-memory token bucket state for one `(tier, source)` pair.
#[derive(Debug, Clone, Copy)]
struct SourceRateBucket {
    tokens: u32,
    fractional_tokens: u64,
    last_refill_at: Instant,
    last_activity_at: Instant,
}

impl SourceRateBucket {
    fn fresh(now: Instant, burst_tokens: u32) -> Self {
        Self {
            tokens: burst_tokens,
            fractional_tokens: 0,
            last_refill_at: now,
            last_activity_at: now,
        }
    }
}

/// Shared request rate limiter used by HTTP and gRPC layers.
///
/// Source-address limits bound unauthenticated churn but cannot distinguish a
/// legitimate newcomer from an attacker controlling many addresses. Sensitive
/// operations must also enforce a limit tied to an authenticated or validated
/// application identity.
#[derive(Debug)]
pub struct RateLimitRegistry {
    tier_policies: Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>>,
    effective_source_caps: Vec<usize>,
    bucket_identity_hasher: RandomState,
    max_live_buckets: usize,
    buckets: Mutex<RateLimitBucketState>,
}

/// Result of an individual rate-limit decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitDecision {
    /// The request is admitted under the policy.
    Allowed,
    /// A per-tier source bucket limit prevented admission.
    SourceLimitReached,
    /// The shared registry cap prevented admission.
    RegistryFull,
}

impl RateLimitRegistry {
    /// Creates a new limiter with shared tier policies.
    pub fn new(tier_policies: Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>>) -> Self {
        Self::new_with_max_live_buckets(tier_policies, RATE_LIMIT_MAX_LIVE_BUCKETS)
    }

    /// Creates a limiter with an explicit registry bucket-cap, useful for tests.
    pub(crate) fn new_with_max_live_buckets(
        tier_policies: Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>>,
        max_live_buckets: usize,
    ) -> Self {
        let configured_source_capacity =
            tier_policies.iter().fold(0_usize, |total, (_, policy)| {
                total.saturating_add(policy.max_distinct_sources())
            });
        if configured_source_capacity > max_live_buckets {
            // Existing policy constructors are infallible at composition time.
            // Reserve finite slots per tier so one overcommitted tier cannot
            // consume another tier's entire source allowance. Newcomers
            // beyond an effective cap use bounded overflow shards.
            tracing::warn!(
                configured_source_capacity,
                max_live_buckets,
                "rate_limit_source_capacity_exceeds_registry"
            );
        }
        Self {
            effective_source_caps: effective_source_caps(
                tier_policies.as_slice(),
                max_live_buckets,
            ),
            tier_policies,
            bucket_identity_hasher: RandomState::new(),
            max_live_buckets,
            buckets: Mutex::new(RateLimitBucketState::default()),
        }
    }

    fn source_bucket_id_with_prefix(
        &self,
        source_identity: RateLimitSourceIdentity,
        ipv6_prefix_len: u8,
    ) -> u64 {
        // SipHash-1-3 with process-randomized state avoids collision attacks
        // against caller-controlled identity values.
        let mut hasher = self.bucket_identity_hasher.build_hasher();
        match source_identity {
            RateLimitSourceIdentity::ForwardedIp(value)
            | RateLimitSourceIdentity::PeerIp(value) => {
                2u8.hash(&mut hasher);
                rate_limit_network_with_prefix(value, ipv6_prefix_len).hash(&mut hasher);
            }
            RateLimitSourceIdentity::Anonymous => {
                3u8.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// Returns the result of applying rate-limit policy for this request.
    pub fn allow(
        &self,
        tier: &HttpRateLimitTierName,
        source_identity: RateLimitSourceIdentity,
    ) -> RateLimitDecision {
        self.allow_at(tier, source_identity, Instant::now())
    }

    fn allow_at(
        &self,
        tier: &HttpRateLimitTierName,
        source_identity: RateLimitSourceIdentity,
        now: Instant,
    ) -> RateLimitDecision {
        let Some((tier_index, policy)) = self
            .tier_policies
            .iter()
            .enumerate()
            .find_map(|(index, (name, policy))| (name == tier).then_some((index, *policy)))
        else {
            // A stale or misspelled tier must never turn a configured guard off.
            return RateLimitDecision::SourceLimitReached;
        };
        if self.max_live_buckets == 0 {
            return RateLimitDecision::RegistryFull;
        }

        let source_bucket = match policy.scope() {
            HttpRateLimitScope::PerSource => {
                self.source_bucket_id_with_prefix(source_identity, policy.ipv6_source_prefix_len())
            }
            HttpRateLimitScope::Shared => 0,
        };

        let mut buckets = recover_rate_limit_buckets_lock(self.buckets.lock());

        let mut use_overflow = false;
        if !buckets.contains(tier, source_bucket) {
            let tier_full = buckets.tier_len(tier) >= self.effective_source_caps[tier_index];
            let registry_full = buckets.len() >= self.max_live_buckets;
            if tier_full || registry_full {
                let evicted = if tier_full {
                    buckets.evict_refilled_tier_bucket(tier, policy, now)
                } else {
                    buckets.evict_refilled_global_bucket(self.tier_policies.as_slice(), now)
                };
                // New sources share one bounded bucket while retained sources
                // still owe rate-limit debt. Churn cannot mint free tokens.
                use_overflow = !evicted;
            }
            if !use_overflow {
                buckets.insert(
                    tier.clone(),
                    source_bucket,
                    SourceRateBucket::fresh(now, policy.burst_tokens()),
                );
            }
        }

        if use_overflow {
            let overflow_burst = policy
                .burst_tokens()
                .saturating_mul(OVERFLOW_BUDGET_MULTIPLIER);
            let shard_count = overflow_burst.min(OVERFLOW_SHARD_LIMIT);
            let Ok(shard) = u32::try_from(source_bucket % u64::from(shard_count)) else {
                return RateLimitDecision::SourceLimitReached;
            };
            let capacity =
                overflow_burst / shard_count + u32::from(shard < overflow_burst % shard_count);
            let decision = consume_overflow_bucket(
                buckets.overflow_bucket(tier, shard, now, capacity),
                policy,
                now,
                shard_count,
                capacity,
            );
            #[cfg(feature = "metrics")]
            record_rate_limit_overflow_decision(decision == RateLimitDecision::Allowed);
            return decision;
        }

        let Some(bucket) = buckets.bucket_mut(tier, source_bucket) else {
            return RateLimitDecision::SourceLimitReached;
        };

        refill_bucket(bucket, policy, now);

        if bucket.tokens == 0 {
            buckets.record_activity(tier, source_bucket, now);
            return RateLimitDecision::SourceLimitReached;
        }

        bucket.tokens = bucket.tokens.saturating_sub(1);
        buckets.record_activity(tier, source_bucket, now);
        RateLimitDecision::Allowed
    }

    /// Removes stale buckets from all tiers.
    pub fn prune_stale_buckets(&self) {
        let mut buckets = recover_rate_limit_buckets_lock(self.buckets.lock());
        buckets.prune_stale(self.tier_policies.as_slice(), Instant::now());
    }

    /// Returns the live bucket count across all tiers.
    pub fn live_bucket_count(&self) -> usize {
        let buckets = recover_rate_limit_buckets_lock(self.buckets.lock());
        buckets.len().saturating_add(buckets.overflow.len())
    }
}

fn effective_source_caps(
    policies: &[(HttpRateLimitTierName, HttpRateLimitTierPolicy)],
    registry_cap: usize,
) -> Vec<usize> {
    let configured_total = policies.iter().fold(0_usize, |total, (_, policy)| {
        total.saturating_add(policy.max_distinct_sources())
    });
    if configured_total <= registry_cap {
        return policies
            .iter()
            .map(|(_, policy)| policy.max_distinct_sources())
            .collect();
    }

    let mut remaining = registry_cap;
    policies
        .iter()
        .enumerate()
        .map(|(index, (_, policy))| {
            let remaining_tiers = policies.len().saturating_sub(index).max(1);
            let fair_share = remaining.div_ceil(remaining_tiers);
            let allocated = policy.max_distinct_sources().min(fair_share);
            remaining = remaining.saturating_sub(allocated);
            allocated
        })
        .collect()
}

fn consume_overflow_bucket(
    bucket: &mut SourceRateBucket,
    policy: HttpRateLimitTierPolicy,
    now: Instant,
    shard_count: u32,
    capacity: u32,
) -> RateLimitDecision {
    refill_bucket_scaled(
        bucket,
        policy,
        now,
        shard_count,
        capacity,
        OVERFLOW_BUDGET_MULTIPLIER,
    );
    bucket.last_activity_at = now;
    if bucket.tokens == 0 {
        return RateLimitDecision::SourceLimitReached;
    }
    bucket.tokens = bucket.tokens.saturating_sub(1);
    RateLimitDecision::Allowed
}

fn refill_bucket(bucket: &mut SourceRateBucket, policy: HttpRateLimitTierPolicy, now: Instant) {
    refill_bucket_scaled(bucket, policy, now, 1, policy.burst_tokens(), 1);
}

fn refill_bucket_scaled(
    bucket: &mut SourceRateBucket,
    policy: HttpRateLimitTierPolicy,
    now: Instant,
    divisor: u32,
    capacity: u32,
    refill_multiplier: u32,
) {
    let elapsed = now.saturating_duration_since(bucket.last_refill_at);
    const NANOS_PER_SECOND: u128 = 1_000_000_000;
    let denominator = NANOS_PER_SECOND.saturating_mul(u128::from(divisor));
    let accrued = elapsed
        .as_nanos()
        .saturating_mul(u128::from(policy.refill_tokens_per_second()))
        .saturating_mul(u128::from(refill_multiplier))
        .saturating_add(u128::from(bucket.fractional_tokens));
    let refill = accrued / denominator;
    let refill_u32 = u32::try_from(refill).unwrap_or(u32::MAX);
    bucket.tokens = bucket.tokens.saturating_add(refill_u32).min(capacity);
    bucket.fractional_tokens = if bucket.tokens == capacity {
        0
    } else {
        u64::try_from(accrued % denominator).unwrap_or_default()
    };
    bucket.last_refill_at = now;
}

pub(crate) fn rate_limit_network(address: IpAddr) -> IpAddr {
    rate_limit_network_with_prefix(address, 64)
}

fn rate_limit_network_with_prefix(address: IpAddr, ipv6_prefix_len: u8) -> IpAddr {
    match address {
        IpAddr::V4(value) => IpAddr::V4(value),
        IpAddr::V6(value) => match value.to_ipv4_mapped() {
            Some(mapped) => IpAddr::V4(mapped),
            None => {
                // A client commonly controls a full IPv6 subnet. Group its
                // addresses under the validated tier prefix.
                let host_bits = 128_u32 - u32::from(ipv6_prefix_len);
                let network = u128::from(value) & (u128::MAX << host_bits);
                IpAddr::V6(Ipv6Addr::from(network))
            }
        },
    }
}

fn recover_rate_limit_buckets_lock(
    result: LockResult<MutexGuard<'_, RateLimitBucketState>>,
) -> MutexGuard<'_, RateLimitBucketState> {
    match result {
        Ok(guard) => guard,
        Err(poisoned) => {
            #[cfg(feature = "metrics")]
            {
                record_rate_limit_mutex_poisoned();
            }
            if !RATE_LIMIT_BUCKET_MUTEX_POISON_WARNED.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    error.kind = "rate_limit_mutex_poisoned",
                    "rate-limit registry bucket mutex was poisoned; recovering inner state"
                );
            }
            poisoned.into_inner()
        }
    }
}

/// Runs request-rate limiter maintenance in a dedicated background task.
///
/// The sweep keeps memory bounded by pruning idle buckets and emits the
/// `rate_limit_buckets_live` metric with the current live bucket cardinality.
pub async fn run_rate_limit_registry_sweep_task(
    registry: Arc<RateLimitRegistry>,
    listener_name: Arc<str>,
    mut shutdown: ShutdownToken,
) -> Result<(), TaskExecutionError> {
    let mut ticker = interval(Duration::from_secs(RATE_LIMIT_BUCKET_SWEEP_INTERVAL_SECS));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                break;
            }
            _ = ticker.tick() => {
                registry.prune_stale_buckets();
                #[cfg(feature = "metrics")]
                {
                    let live_buckets = registry.live_bucket_count();
                    record_rate_limit_buckets_live(Arc::clone(&listener_name), live_buckets);
                }
            }
        }
    }
    #[cfg(feature = "metrics")]
    {
        let live_buckets = registry.live_bucket_count();
        record_rate_limit_buckets_live(listener_name, live_buckets);
    }
    Ok(())
}

#[cfg(test)]
#[path = "rate_limit_tests.rs"]
mod tests;
