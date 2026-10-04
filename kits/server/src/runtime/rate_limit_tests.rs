// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::RateLimitSourceIdentity;
use crate::http::HttpRateLimitTierName;
use crate::runtime::{HttpRateLimitScope, HttpRateLimitTierPolicy};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn indexes_are_consistent(state: &super::state::RateLimitBucketState) -> bool {
    super::state::tests::indexes_are_consistent(state)
}

fn source_bucket_id(
    registry: &super::RateLimitRegistry,
    source_identity: RateLimitSourceIdentity,
) -> u64 {
    registry.source_bucket_id_with_prefix(source_identity, 64)
}

fn source(number: u8) -> RateLimitSourceIdentity {
    RateLimitSourceIdentity::PeerIp(std::net::IpAddr::V4(std::net::Ipv4Addr::new(
        192, 0, 2, number,
    )))
}

fn tier_map_count(registry: &super::RateLimitRegistry) -> usize {
    let buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
    buckets.by_tier.len()
}

#[test]
fn scope_specific_sources_share_bucket_map_entry_limit() {
    let tier = HttpRateLimitTierName::new("shared").expect("tier name should be valid");
    let policy = HttpRateLimitTierPolicy::new(1, 1, 1)
        .expect("valid rate-limit tier policy")
        .with_scope(HttpRateLimitScope::Shared);
    let registry = super::RateLimitRegistry::new(Arc::new(vec![(tier.clone(), policy)]));

    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(1))
    );
    assert_eq!(
        super::RateLimitDecision::SourceLimitReached,
        registry.allow(&tier, source(2))
    );
}

#[test]
fn sweep_reduces_bucket_count_after_ttl() {
    let stale_at = Instant::now() - Duration::from_secs(super::RATE_LIMIT_BUCKET_IDLE_TTL_SECS + 1);
    let tier = HttpRateLimitTierName::new("ttl-test").expect("tier name should be valid");
    let mut buckets = super::state::RateLimitBucketState::default();
    buckets.insert(
        tier.clone(),
        1,
        super::SourceRateBucket {
            tokens: 0,
            fractional_tokens: 0,
            last_refill_at: stale_at,
            last_activity_at: stale_at,
        },
    );
    let policy = HttpRateLimitTierPolicy::new(1, 1, 1).expect("valid policy");
    buckets.prune_stale(&[(tier, policy)], Instant::now());

    assert_eq!(buckets.len(), 0);
    assert!(buckets.by_tier.is_empty());
    assert!(indexes_are_consistent(&buckets));
}

#[test]
fn sweep_keeps_stale_source_debt_until_its_bucket_is_fully_refilled() {
    let now = Instant::now();
    let stale_at = now - Duration::from_secs(super::RATE_LIMIT_BUCKET_IDLE_TTL_SECS + 1);
    let tier = HttpRateLimitTierName::new("debt-ttl").expect("valid tier");
    let policy = HttpRateLimitTierPolicy::new(1, 10_000, 1).expect("valid policy");
    let mut buckets = super::state::RateLimitBucketState::default();
    buckets.insert(
        tier.clone(),
        1,
        super::SourceRateBucket {
            tokens: 0,
            fractional_tokens: 0,
            last_refill_at: stale_at,
            last_activity_at: stale_at,
        },
    );
    buckets.overflow_bucket(&tier, stale_at).tokens = 0;

    buckets.prune_stale(&[(tier.clone(), policy)], now);
    assert!(
        buckets.contains(&tier, 1),
        "token debt must survive the idle sweep"
    );
    assert!(buckets.overflow.contains_key(&tier));
    assert!(indexes_are_consistent(&buckets));

    buckets.prune_stale(&[(tier.clone(), policy)], now + Duration::from_secs(10_000));
    assert!(
        !buckets.contains(&tier, 1),
        "fully refilled debt may be evicted"
    );
    assert!(!buckets.overflow.contains_key(&tier));
    assert!(indexes_are_consistent(&buckets));
}

#[test]
fn source_bucket_ids_isolate_anonymous_from_network_sources() {
    let tier = HttpRateLimitTierName::new("isolation").expect("tier name should be valid");
    let registry = super::RateLimitRegistry::new(Arc::new(vec![(
        tier.clone(),
        HttpRateLimitTierPolicy::new(1, 1, 10).expect("valid rate-limit tier policy"),
    )]));

    let anonymous_id = source_bucket_id(&registry, RateLimitSourceIdentity::Anonymous);
    let peer_ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
    let peer_id = source_bucket_id(&registry, RateLimitSourceIdentity::PeerIp(peer_ip));

    assert_ne!(anonymous_id, peer_id);
}

#[test]
fn ip_sources_share_buckets_across_ipv6_interface_ids_and_mapped_ipv4() {
    let registry = super::RateLimitRegistry::new(Arc::new(Vec::new()));
    let first: std::net::IpAddr = "2001:db8:1:2::1".parse().expect("valid IPv6 fixture");
    let second: std::net::IpAddr = "2001:db8:1:2::ffff".parse().expect("valid IPv6 fixture");
    let other: std::net::IpAddr = "2001:db8:1:3::1".parse().expect("valid IPv6 fixture");
    assert_eq!(
        source_bucket_id(&registry, RateLimitSourceIdentity::ForwardedIp(first)),
        source_bucket_id(&registry, RateLimitSourceIdentity::PeerIp(second))
    );
    assert_ne!(
        source_bucket_id(&registry, RateLimitSourceIdentity::PeerIp(first)),
        source_bucket_id(&registry, RateLimitSourceIdentity::PeerIp(other))
    );

    let ipv4: std::net::IpAddr = "192.0.2.4".parse().expect("valid IPv4 fixture");
    let mapped: std::net::IpAddr = "::ffff:192.0.2.4".parse().expect("valid mapped fixture");
    assert_eq!(
        source_bucket_id(&registry, RateLimitSourceIdentity::PeerIp(ipv4)),
        source_bucket_id(&registry, RateLimitSourceIdentity::ForwardedIp(mapped))
    );
}

#[test]
fn rotating_ipv6_interface_ids_cannot_exhaust_source_registry() {
    let tier = HttpRateLimitTierName::new("ipv6-source").expect("valid tier fixture");
    let policy = HttpRateLimitTierPolicy::new(1, 1, 2).expect("valid policy fixture");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(tier.clone(), policy)]),
        2,
    );

    for suffix in 0..1_000_u32 {
        let address = std::net::Ipv6Addr::from(
            (u128::from(0x2001_0db8_0001_0002_u64) << 64) | u128::from(suffix),
        );
        let expected = if suffix == 0 {
            super::RateLimitDecision::Allowed
        } else {
            super::RateLimitDecision::SourceLimitReached
        };
        assert_eq!(
            registry.allow(&tier, RateLimitSourceIdentity::PeerIp(address.into())),
            expected,
        );
    }
    assert_eq!(registry.live_bucket_count(), 1);
    let other: std::net::IpAddr = "192.0.2.7".parse().expect("valid client fixture");
    assert_eq!(
        registry.allow(&tier, RateLimitSourceIdentity::PeerIp(other)),
        super::RateLimitDecision::Allowed,
    );
}

#[test]
fn fractional_refill_preserves_subsecond_credit() {
    let policy = HttpRateLimitTierPolicy::new(1, 2, 1).expect("valid policy");
    let start = Instant::now();
    let mut bucket = super::SourceRateBucket::fresh(start, 2);
    bucket.tokens = 0;

    super::refill_bucket(&mut bucket, policy, start + Duration::from_millis(900));
    assert_eq!(bucket.tokens, 0);
    super::refill_bucket(&mut bucket, policy, start + Duration::from_millis(1_100));
    assert_eq!(bucket.tokens, 1);
    assert_eq!(bucket.fractional_tokens, 100_000_000);
}

#[test]
fn source_bucket_id_is_deterministic_within_registry() {
    let tier = HttpRateLimitTierName::new("deterministic").expect("tier name should be valid");
    let registry = super::RateLimitRegistry::new(Arc::new(vec![(
        tier.clone(),
        HttpRateLimitTierPolicy::new(1, 1, 10).expect("valid rate-limit tier policy"),
    )]));

    let first = source_bucket_id(&registry, source(1));
    let second = source_bucket_id(&registry, source(1));

    assert_eq!(first, second);
}

#[test]
fn source_bucket_ids_vary_between_registries() {
    let tier = HttpRateLimitTierName::new("registry-isolation").expect("tier name should be valid");
    let policy = HttpRateLimitTierPolicy::new(1, 1, 10).expect("valid rate-limit tier policy");
    let registry_a = super::RateLimitRegistry::new(Arc::new(vec![(tier.clone(), policy)]));
    let registry_b = super::RateLimitRegistry::new(Arc::new(vec![(tier, policy)]));

    let first = source_bucket_id(&registry_a, source(1));
    let second = source_bucket_id(&registry_b, source(1));

    assert_ne!(
        first, second,
        "distinct registries should keep separate hash state for isolated map growth"
    );
}

#[test]
fn global_cap_routes_new_tier_through_bounded_overflow() {
    let tier_a = HttpRateLimitTierName::new("tier-a").expect("tier name should be valid");
    let tier_b = HttpRateLimitTierName::new("tier-b").expect("tier name should be valid");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![
            (
                tier_a.clone(),
                HttpRateLimitTierPolicy::new(1, 1, 10).expect("valid rate-limit tier policy"),
            ),
            (
                tier_b.clone(),
                HttpRateLimitTierPolicy::new(1, 1, 10).expect("valid rate-limit tier policy"),
            ),
        ]),
        1,
    );

    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier_a, source(1))
    );
    assert_eq!(1, tier_map_count(&registry));
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier_b, source(2))
    );
    assert_eq!(2, registry.live_bucket_count());
    assert_eq!(1, tier_map_count(&registry));
}

#[test]
fn zero_registry_capacity_fails_closed() {
    let tier = HttpRateLimitTierName::new("zero-capacity").expect("valid tier");
    let policy = HttpRateLimitTierPolicy::new(1, 1, 1).expect("valid policy");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(tier.clone(), policy)]),
        0,
    );
    assert_eq!(
        super::RateLimitDecision::RegistryFull,
        registry.allow(&tier, source(1))
    );
    assert_eq!(registry.live_bucket_count(), 0);
}

#[test]
fn registry_full_does_not_forgive_source_token_debt() {
    let tier = HttpRateLimitTierName::new("registry-full").expect("tier name should be valid");
    let policy = HttpRateLimitTierPolicy::new(1, 10, 10)
        .expect("valid rate-limit tier policy")
        .with_scope(HttpRateLimitScope::PerSource);
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(tier.clone(), policy)]),
        1,
    );

    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(1))
    );
    assert_eq!(1, registry.live_bucket_count());
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(2))
    );
    assert_eq!(
        super::RateLimitDecision::SourceLimitReached,
        registry.allow(&tier, source(3)),
        "newcomers share the same initial token"
    );
    {
        let mut buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
        let overflow = buckets.overflow.get_mut(&tier).expect("overflow exists");
        overflow.last_refill_at -= Duration::from_secs(1);
    }
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(3)),
        "new sources share the configured refill rate"
    );
    assert_eq!(
        super::RateLimitDecision::SourceLimitReached,
        registry.allow(&tier, source(4)),
        "another identity cannot obtain a fresh overflow token"
    );
    assert_eq!(2, registry.live_bucket_count());
    assert_eq!(1, tier_map_count(&registry));
}

#[test]
fn full_tier_preserves_throttled_source_debt() {
    let tier = HttpRateLimitTierName::new("tier-lru").expect("valid tier");
    let policy = HttpRateLimitTierPolicy::new(1, 3, 2).expect("valid policy");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(tier.clone(), policy)]),
        2,
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(1))
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(2))
    );
    let first_id = source_bucket_id(&registry, source(1));
    let second_id = source_bucket_id(&registry, source(2));
    let third_id = source_bucket_id(&registry, source(3));
    std::thread::sleep(Duration::from_millis(1));
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(1)),
        "admitting the first source refreshes its eviction priority"
    );
    {
        let mut buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
        buckets
            .bucket_mut(&tier, second_id)
            .expect("second source exists")
            .tokens = 0;
    }
    assert_eq!(
        super::RateLimitDecision::SourceLimitReached,
        registry.allow(&tier, source(2)),
        "denied traffic retains the source's token debt"
    );

    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(3))
    );
    let buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
    let entries = buckets.by_tier.get(&tier).expect("tier remains");
    assert_eq!(entries.len(), 2);
    assert!(entries.contains_key(&first_id));
    assert!(entries.contains_key(&second_id));
    assert!(!entries.contains_key(&third_id));
    assert_eq!(buckets.overflow.len(), 1);
    assert!(indexes_are_consistent(&buckets));
}

#[test]
fn refilled_source_can_be_replaced_without_forgiving_rate_limit_debt() {
    let tier = HttpRateLimitTierName::new("safe-replacement").expect("valid tier");
    let policy = HttpRateLimitTierPolicy::new(1, 1, 1).expect("valid policy");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(tier.clone(), policy)]),
        1,
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(1))
    );
    {
        let mut buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
        let first = source_bucket_id(&registry, source(1));
        let bucket = buckets.bucket_mut(&tier, first).expect("source exists");
        bucket.last_refill_at -= Duration::from_secs(1);
    }
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(2))
    );
    assert_eq!(registry.live_bucket_count(), 1);
    let buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
    assert!(!buckets.contains(&tier, source_bucket_id(&registry, source(1))));
    assert!(buckets.contains(&tier, source_bucket_id(&registry, source(2))));
    assert!(indexes_are_consistent(&buckets));
}

#[test]
fn full_tier_replaces_a_refilled_source_behind_an_older_debtor() {
    let tier = HttpRateLimitTierName::new("refilled-candidate").expect("valid tier");
    let policy = HttpRateLimitTierPolicy::new(1, 2, 2).expect("valid policy");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(tier.clone(), policy)]),
        2,
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(1))
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(2))
    );
    let first_id = source_bucket_id(&registry, source(1));
    let second_id = source_bucket_id(&registry, source(2));
    {
        let mut buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
        buckets
            .bucket_mut(&tier, second_id)
            .expect("second source exists")
            .last_refill_at -= Duration::from_secs(1);
    }

    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, source(3))
    );
    let buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
    assert!(
        buckets.contains(&tier, first_id),
        "the indebted source remains"
    );
    assert!(
        !buckets.contains(&tier, second_id),
        "the refilled source makes room"
    );
    assert!(buckets.contains(&tier, source_bucket_id(&registry, source(3))));
    assert!(indexes_are_consistent(&buckets));
}

#[test]
fn full_registry_replaces_a_refilled_source_behind_an_older_debtor() {
    let first = HttpRateLimitTierName::new("older-debtor-tier").expect("valid tier");
    let second = HttpRateLimitTierName::new("newcomer-tier").expect("valid tier");
    let policy = HttpRateLimitTierPolicy::new(1, 2, 2).expect("valid policy");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(first.clone(), policy), (second.clone(), policy)]),
        2,
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&first, source(1))
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&first, source(2))
    );
    let first_id = source_bucket_id(&registry, source(1));
    let second_id = source_bucket_id(&registry, source(2));
    {
        let mut buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
        buckets
            .bucket_mut(&first, second_id)
            .expect("second source exists")
            .last_refill_at -= Duration::from_secs(1);
    }

    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&second, source(3))
    );
    let buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
    assert!(
        buckets.contains(&first, first_id),
        "the indebted source remains"
    );
    assert!(
        !buckets.contains(&first, second_id),
        "the refilled source makes room"
    );
    assert!(buckets.contains(&second, source_bucket_id(&registry, source(3))));
    assert!(indexes_are_consistent(&buckets));
}

#[test]
fn rotating_one_more_source_than_the_tier_tracks_cannot_bypass_the_limit() {
    let tier = HttpRateLimitTierName::new("rotating-sources").expect("valid tier");
    let policy = HttpRateLimitTierPolicy::new(1, 1, 2).expect("valid policy");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(tier.clone(), policy)]),
        2,
    );

    let admitted = (0..300)
        .filter(|attempt| {
            registry.allow(
                &tier,
                source(u8::try_from(attempt % 3 + 1).expect("small source")),
            ) == super::RateLimitDecision::Allowed
        })
        .count();
    assert!(admitted < 10, "identity rotation must not mint free tokens");
    assert_eq!(registry.live_bucket_count(), 3);
}

#[test]
fn global_cap_preserves_existing_source_buckets() {
    let first = HttpRateLimitTierName::new("first").expect("valid tier");
    let second = HttpRateLimitTierName::new("second").expect("valid tier");
    let policy = HttpRateLimitTierPolicy::new(1, 2, 3).expect("valid policy");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(first.clone(), policy), (second.clone(), policy)]),
        2,
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&first, source(1))
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&first, source(2))
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&second, source(3))
    );

    let buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
    assert_eq!(buckets.by_tier.get(&first).expect("first tier").len(), 2);
    assert!(!buckets.by_tier.contains_key(&second));
    assert!(buckets.overflow.contains_key(&second));
    assert!(indexes_are_consistent(&buckets));
}

#[test]
fn global_cap_does_not_evict_unreplenished_sources() {
    let first = HttpRateLimitTierName::new("global-first").expect("valid tier");
    let second = HttpRateLimitTierName::new("global-second").expect("valid tier");
    let policy = HttpRateLimitTierPolicy::new(1, 3, 3).expect("valid policy");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(first.clone(), policy), (second.clone(), policy)]),
        3,
    );
    for (tier, identity) in [
        (&first, source(1)),
        (&first, source(2)),
        (&second, source(3)),
    ] {
        assert_eq!(
            super::RateLimitDecision::Allowed,
            registry.allow(tier, identity)
        );
    }
    std::thread::sleep(Duration::from_millis(1));
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&first, source(1))
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&second, source(4))
    );

    let buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
    let first_entries = buckets.by_tier.get(&first).expect("first tier remains");
    let second_entries = buckets.by_tier.get(&second).expect("second tier remains");
    assert!(first_entries.contains_key(&source_bucket_id(&registry, source(1))));
    assert!(first_entries.contains_key(&source_bucket_id(&registry, source(2))));
    assert_eq!(second_entries.len(), 1);
    assert!(buckets.overflow.contains_key(&second));
    assert!(indexes_are_consistent(&buckets));
}

#[test]
fn rotating_ipv6_networks_share_a_rate_limited_overflow() {
    let tier = HttpRateLimitTierName::new("ipv6-rotation").expect("valid tier");
    const LIVE_BUCKET_LIMIT: usize = 2_048;
    let policy = HttpRateLimitTierPolicy::new(1, 4, LIVE_BUCKET_LIMIT).expect("valid policy");
    let registry = super::RateLimitRegistry::new_with_max_live_buckets(
        Arc::new(vec![(tier.clone(), policy)]),
        LIVE_BUCKET_LIMIT,
    );

    let mut allowed = 0_usize;
    for network in 0..32_768_u128 {
        let address = std::net::Ipv6Addr::from(
            (u128::from(0x2001_0db8_0001_u64) << 80) | (network << 64) | 1,
        );
        allowed += usize::from(
            registry.allow(&tier, RateLimitSourceIdentity::PeerIp(address.into()))
                == super::RateLimitDecision::Allowed,
        );
        assert!(registry.live_bucket_count() <= LIVE_BUCKET_LIMIT + 1);
    }

    assert!(
        allowed < 4_096,
        "source rotation cannot mint a token per identity"
    );
    let new_client: std::net::IpAddr = "192.0.2.200".parse().expect("valid IPv4 fixture");
    {
        let mut buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
        let overflow = buckets.overflow.get_mut(&tier).expect("overflow exists");
        overflow.last_refill_at -= Duration::from_secs(1);
    }
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, RateLimitSourceIdentity::PeerIp(new_client))
    );
    assert_eq!(registry.live_bucket_count(), LIVE_BUCKET_LIMIT + 1);
    let buckets = super::recover_rate_limit_buckets_lock(registry.buckets.lock());
    assert!(indexes_are_consistent(&buckets));
}

#[test]
fn configured_ipv6_prefix_groups_multiple_networks_in_one_source_bucket() {
    let tier = HttpRateLimitTierName::new("ipv6-prefix").expect("valid tier");
    let policy = HttpRateLimitTierPolicy::new(1, 1, 2)
        .expect("valid policy")
        .with_ipv6_source_prefix_len(48)
        .expect("valid prefix");
    let registry = super::RateLimitRegistry::new(Arc::new(vec![(tier.clone(), policy)]));
    let first: std::net::IpAddr = "2001:db8:1:2::1".parse().expect("valid IPv6 fixture");
    let same_prefix: std::net::IpAddr = "2001:db8:1:3::1".parse().expect("valid IPv6 fixture");
    let other_prefix: std::net::IpAddr = "2001:db8:2:1::1".parse().expect("valid IPv6 fixture");

    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, RateLimitSourceIdentity::PeerIp(first))
    );
    assert_eq!(
        super::RateLimitDecision::SourceLimitReached,
        registry.allow(&tier, RateLimitSourceIdentity::PeerIp(same_prefix))
    );
    assert_eq!(
        super::RateLimitDecision::Allowed,
        registry.allow(&tier, RateLimitSourceIdentity::PeerIp(other_prefix))
    );
    assert_eq!(registry.live_bucket_count(), 2);
}
