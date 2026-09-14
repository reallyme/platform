// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::hash_map::{HashMap, RandomState};
use std::hash::{BuildHasher, Hash, Hasher};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LockResult, Mutex, MutexGuard};
use std::time::Instant;

use tokio::time::{Duration, MissedTickBehavior, interval};

use crate::http::HttpRateLimitTierName;
#[cfg(feature = "metrics")]
use crate::observability::{record_rate_limit_buckets_live, record_rate_limit_mutex_poisoned};
use crate::runtime::{HttpRateLimitScope, HttpRateLimitTierPolicy};
use crate::task::{ShutdownToken, TaskExecutionError};

/// Maximum number of live rate-limit buckets per registry.
pub const RATE_LIMIT_MAX_LIVE_BUCKETS: usize = 25_000;

/// Idle bucket pruning window for sweep and request-time eviction.
pub const RATE_LIMIT_BUCKET_IDLE_TTL_SECS: u64 = 900;

/// Periodic sweep cadence used by runtime-managed background tasks.
pub const RATE_LIMIT_BUCKET_SWEEP_INTERVAL_SECS: u64 = 60;

static RATE_LIMIT_BUCKET_MUTEX_POISON_WARNED: AtomicBool = AtomicBool::new(false);

/// Low-cost source identity used to compute per-source bucket keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitSourceIdentity<'a> {
    /// Request principal header supplied by upstream auth adapters.
    Principal(&'a str),
    /// Authenticated API key identity supplied by upstream auth adapters.
    ApiKey(&'a str),
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
    last_refill_at: Instant,
    last_activity_at: Instant,
}

impl SourceRateBucket {
    fn fresh(now: Instant, burst_tokens: u32) -> Self {
        Self {
            tokens: burst_tokens,
            last_refill_at: now,
            last_activity_at: now,
        }
    }
}

type RateLimitBucketMap = HashMap<HttpRateLimitTierName, HashMap<u64, SourceRateBucket>>;

/// Shared request rate limiter used by HTTP and gRPC layers.
#[derive(Debug)]
pub struct RateLimitRegistry {
    tier_policies: Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>>,
    bucket_identity_hasher: RandomState,
    max_live_buckets: usize,
    buckets: Mutex<RateLimitBucketMap>,
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
        Self {
            tier_policies,
            bucket_identity_hasher: RandomState::new(),
            max_live_buckets,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    fn source_bucket_id(&self, source_identity: RateLimitSourceIdentity<'_>) -> u64 {
        // SipHash-1-3 with process-randomized state avoids collision attacks
        // against caller-controlled identity values.
        let mut hasher = self.bucket_identity_hasher.build_hasher();
        match source_identity {
            RateLimitSourceIdentity::Principal(value) => {
                0u8.hash(&mut hasher);
                value.hash(&mut hasher);
            }
            RateLimitSourceIdentity::ApiKey(value) => {
                1u8.hash(&mut hasher);
                value.hash(&mut hasher);
            }
            RateLimitSourceIdentity::ForwardedIp(value) => {
                2u8.hash(&mut hasher);
                value.hash(&mut hasher);
            }
            RateLimitSourceIdentity::PeerIp(value) => {
                3u8.hash(&mut hasher);
                value.hash(&mut hasher);
            }
            RateLimitSourceIdentity::Anonymous => {
                4u8.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// Returns the result of applying rate-limit policy for this request.
    pub fn allow<'a>(
        &self,
        tier: &HttpRateLimitTierName,
        source_identity: RateLimitSourceIdentity<'a>,
    ) -> RateLimitDecision {
        let Some(policy) = self
            .tier_policies
            .iter()
            .find_map(|(name, policy)| (name == tier).then_some(*policy))
        else {
            return RateLimitDecision::Allowed;
        };

        let source_bucket = match policy.scope() {
            HttpRateLimitScope::PerSource => self.source_bucket_id(source_identity),
            HttpRateLimitScope::Shared => 0,
        };

        let now = Instant::now();
        let mut buckets = recover_rate_limit_buckets_lock(self.buckets.lock());

        let mut creating_new_bucket = false;
        {
            if let Some(tier_buckets) = buckets.get_mut(tier) {
                if !tier_buckets.contains_key(&source_bucket) {
                    prune_tier_buckets_locked(tier_buckets, now);
                    if tier_buckets.len() >= policy.max_distinct_sources() {
                        return RateLimitDecision::SourceLimitReached;
                    }
                    creating_new_bucket = true;
                }
            } else {
                if policy.max_distinct_sources() == 0 {
                    return RateLimitDecision::SourceLimitReached;
                }
                if self.total_bucket_count_locked(&buckets) >= self.max_live_buckets {
                    return RateLimitDecision::RegistryFull;
                }
                buckets.insert(tier.clone(), HashMap::new());
                creating_new_bucket = true;
            }
        }

        if creating_new_bucket && self.total_bucket_count_locked(&buckets) >= self.max_live_buckets
        {
            return RateLimitDecision::RegistryFull;
        }

        let Some(tier_buckets) = buckets.get_mut(tier) else {
            return RateLimitDecision::SourceLimitReached;
        };
        let bucket = tier_buckets
            .entry(source_bucket)
            .or_insert_with(|| SourceRateBucket::fresh(now, policy.burst_tokens()));

        refill_bucket(bucket, policy, now);

        if bucket.tokens == 0 {
            return RateLimitDecision::SourceLimitReached;
        }

        bucket.tokens = bucket.tokens.saturating_sub(1);
        bucket.last_activity_at = now;
        RateLimitDecision::Allowed
    }

    /// Removes stale buckets from all tiers.
    pub fn prune_stale_buckets(&self) {
        let mut buckets = recover_rate_limit_buckets_lock(self.buckets.lock());
        prune_stale_buckets_locked(&mut buckets, Instant::now());
    }

    /// Returns the live bucket count across all tiers.
    pub fn live_bucket_count(&self) -> usize {
        let buckets = recover_rate_limit_buckets_lock(self.buckets.lock());
        self.total_bucket_count_locked(&buckets)
    }

    fn total_bucket_count_locked(&self, buckets: &RateLimitBucketMap) -> usize {
        buckets.values().map(|entries| entries.len()).sum()
    }
}

fn prune_tier_buckets_locked(tier_buckets: &mut HashMap<u64, SourceRateBucket>, now: Instant) {
    tier_buckets.retain(|_, bucket| {
        now.saturating_duration_since(bucket.last_activity_at)
            .as_secs()
            < RATE_LIMIT_BUCKET_IDLE_TTL_SECS
    });
}

fn prune_stale_buckets_locked(buckets: &mut RateLimitBucketMap, now: Instant) {
    buckets.retain(|_, tier_buckets| {
        prune_tier_buckets_locked(tier_buckets, now);
        !tier_buckets.is_empty()
    });
}

fn refill_bucket(bucket: &mut SourceRateBucket, policy: HttpRateLimitTierPolicy, now: Instant) {
    let elapsed = now.saturating_duration_since(bucket.last_refill_at);
    let refill = elapsed
        .as_secs()
        .saturating_mul(u64::from(policy.refill_tokens_per_second()));
    if refill == 0 {
        return;
    }

    let refill_u32 = u32::try_from(refill).unwrap_or(u32::MAX);
    bucket.tokens = bucket
        .tokens
        .saturating_add(refill_u32)
        .min(policy.burst_tokens());
    bucket.last_refill_at = now;
}

fn recover_rate_limit_buckets_lock(
    result: LockResult<MutexGuard<'_, RateLimitBucketMap>>,
) -> MutexGuard<'_, RateLimitBucketMap> {
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
