// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::RateLimitSourceIdentity;
use crate::http::HttpRateLimitTierName;
use crate::runtime::{HttpRateLimitScope, HttpRateLimitTierPolicy};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn source_bucket_id(
    registry: &super::RateLimitRegistry,
    source_identity: RateLimitSourceIdentity<'_>,
) -> u64 {
    registry.source_bucket_id(source_identity)
}

fn tier_map_count(registry: &super::RateLimitRegistry) -> usize {
    let buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
    buckets.len()
}

#[test]
fn scope_specific_sources_share_bucket_map_entry_limit() {
    let tier = HttpRateLimitTierName::new("shared").expect("tier name should be valid");
    let policy = HttpRateLimitTierPolicy::new(0, 1, 1)
        .expect("valid rate-limit tier policy")
        .with_scope(HttpRateLimitScope::Shared);
    let registry = super::RateLimitRegistry::new(Arc::new(vec![(tier.clone(), policy)]));

    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, RateLimitSourceIdentity::ApiKey("first"))
    );
    assert_eq!(
        super::RateLimitDecision::SourceLimitReached,
        registry.allow(&tier, RateLimitSourceIdentity::ApiKey("second"))
    );
}

#[test]
fn sweep_reduces_bucket_count_after_ttl() {
    let stale_at = Instant::now() - Duration::from_secs(super::RATE_LIMIT_BUCKET_IDLE_TTL_SECS + 1);
    let tier = HttpRateLimitTierName::new("ttl-test").expect("tier name should be valid");
    let mut tier_buckets = HashMap::new();
    tier_buckets.insert(
        1,
        super::SourceRateBucket {
            tokens: 0,
            last_refill_at: stale_at,
            last_activity_at: stale_at,
        },
    );
    let mut buckets = HashMap::new();
    buckets.insert(tier, tier_buckets);

    super::prune_stale_buckets_locked(&mut buckets, Instant::now());

    assert!(buckets.is_empty());
}

#[test]
fn source_bucket_ids_isolate_distinct_identity_types() {
    let tier = HttpRateLimitTierName::new("isolation").expect("tier name should be valid");
    let registry = super::RateLimitRegistry::new(Arc::new(vec![(
        tier.clone(),
        HttpRateLimitTierPolicy::new(0, 1, 10).expect("valid rate-limit tier policy"),
    )]));

    let principal_id = source_bucket_id(&registry, RateLimitSourceIdentity::Principal("principal"));
    let api_key_id = source_bucket_id(&registry, RateLimitSourceIdentity::ApiKey("principal"));
    let peer_ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
    let peer_id = source_bucket_id(&registry, RateLimitSourceIdentity::PeerIp(peer_ip));

    assert_ne!(principal_id, api_key_id);
    assert_ne!(principal_id, peer_id);
    assert_ne!(api_key_id, peer_id);
}

#[test]
fn source_bucket_id_is_deterministic_within_registry() {
    let tier = HttpRateLimitTierName::new("deterministic").expect("tier name should be valid");
    let registry = super::RateLimitRegistry::new(Arc::new(vec![(
        tier.clone(),
        HttpRateLimitTierPolicy::new(0, 1, 10).expect("valid rate-limit tier policy"),
    )]));

    let first = source_bucket_id(&registry, RateLimitSourceIdentity::Principal("principal"));
    let second = source_bucket_id(&registry, RateLimitSourceIdentity::Principal("principal"));

    assert_eq!(first, second);
}

#[test]
fn source_bucket_ids_vary_between_registries() {
    let tier = HttpRateLimitTierName::new("registry-isolation").expect("tier name should be valid");
    let policy = HttpRateLimitTierPolicy::new(0, 1, 10).expect("valid rate-limit tier policy");
    let registry_a = super::RateLimitRegistry::new(Arc::new(vec![(tier.clone(), policy)]));
    let registry_b = super::RateLimitRegistry::new(Arc::new(vec![(tier, policy)]));

    let first = source_bucket_id(&registry_a, RateLimitSourceIdentity::Principal("principal"));
    let second = source_bucket_id(&registry_b, RateLimitSourceIdentity::Principal("principal"));

    assert_ne!(
        first, second,
        "distinct registries should keep separate hash state for isolated map growth"
    );
}

#[test]
fn global_cap_rejects_new_tier_without_expanding_live_tier_maps() {
    let tier_a = HttpRateLimitTierName::new("tier-a").expect("tier name should be valid");
    let tier_b = HttpRateLimitTierName::new("tier-b").expect("tier name should be valid");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![
            (
                tier_a.clone(),
                HttpRateLimitTierPolicy::new(0, 1, 10).expect("valid rate-limit tier policy"),
            ),
            (
                tier_b.clone(),
                HttpRateLimitTierPolicy::new(0, 1, 10).expect("valid rate-limit tier policy"),
            ),
        ]),
        1,
    );

    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier_a, RateLimitSourceIdentity::ApiKey("first"))
    );
    assert_eq!(1, tier_map_count(&registry));
    assert_eq!(
        super::RateLimitDecision::RegistryFull,
        registry.allow(&tier_b, RateLimitSourceIdentity::ApiKey("second"))
    );
    assert_eq!(1, registry.live_bucket_count());
    assert_eq!(1, tier_map_count(&registry));
}

#[test]
fn registry_full_rejects_new_source_without_request_time_sweep() {
    let tier = HttpRateLimitTierName::new("registry-full").expect("tier name should be valid");
    let policy = HttpRateLimitTierPolicy::new(0, 10, 10)
        .expect("valid rate-limit tier policy")
        .with_scope(HttpRateLimitScope::PerSource);
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(tier.clone(), policy)]),
        1,
    );

    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, RateLimitSourceIdentity::ApiKey("first"))
    );
    assert_eq!(1, registry.live_bucket_count());
    assert_eq!(
        super::RateLimitDecision::RegistryFull,
        registry.allow(&tier, RateLimitSourceIdentity::ApiKey("second"))
    );
    assert_eq!(1, registry.live_bucket_count());
    assert_eq!(1, tier_map_count(&registry));
}
