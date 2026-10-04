// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Router;
use std::sync::Arc;
use thiserror::Error;

use crate::config::HttpServerConfig;
use crate::http::{
    HttpListenerName, HttpListenerVisibility, HttpRateLimitTierName, HttpRouteVisibilityPolicy,
};

#[path = "http/serve.rs"]
mod serve;

pub(crate) use serve::{HttpServePolicy, serve_http};

/// HTTP server input owned by server composition and run by [`crate::runtime::ServerRuntime`].
pub struct HttpServerSpec {
    name: HttpListenerName,
    visibility: HttpListenerVisibility,
    config: HttpServerConfig,
    app_router: Router,
    route_visibility_policy: HttpRouteVisibilityPolicy,
    rate_limit_tier: Option<HttpRateLimitTierName>,
    rate_limit_policies: Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>>,
}

/// Distinct source-bucket scope for one rate-limit tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRateLimitScope {
    /// Keep independent buckets for each derived request source.
    PerSource,
    /// Share one bucket across all request sources assigned to the tier.
    Shared,
}

/// Token-bucket policy parameters for one HTTP rate-limit tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpRateLimitTierPolicy {
    refill_tokens_per_second: u32,
    burst_tokens: u32,
    max_distinct_sources: usize,
    scope: HttpRateLimitScope,
    ipv6_source_prefix_len: u8,
}

/// Low-cardinality reason a rate-limit tier policy was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRateLimitTierPolicyErrorReason {
    /// A bucket without refill permanently denies a source after its first burst.
    ZeroRefillTokens,
    /// A token bucket with no burst capacity can never admit traffic.
    ZeroBurstTokens,
    /// Per-source limiting requires at least one retained source bucket.
    ZeroMaxDistinctSources,
    /// Refilling more than the bucket can hold wastes work and obscures intent.
    RefillExceedsBurst,
}

/// Typed validation error for rate-limit tier policy construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid HTTP rate-limit tier policy: {reason:?}")]
pub struct HttpRateLimitTierPolicyError {
    reason: HttpRateLimitTierPolicyErrorReason,
}

impl HttpRateLimitTierPolicyError {
    const fn new(reason: HttpRateLimitTierPolicyErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the low-cardinality rejection reason.
    pub const fn reason(self) -> HttpRateLimitTierPolicyErrorReason {
        self.reason
    }
}

/// Low-cardinality reason an IPv6 rate-limit source prefix was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpIpv6SourcePrefixErrorReason {
    /// Prefixes outside /48 through /128 either over-group clients or are invalid.
    OutsideSupportedRange,
}

/// Typed validation error for per-source IPv6 rate-limit grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid IPv6 rate-limit source prefix: {reason:?}")]
pub struct HttpIpv6SourcePrefixError {
    reason: HttpIpv6SourcePrefixErrorReason,
}

impl HttpIpv6SourcePrefixError {
    /// Returns the low-cardinality rejection reason.
    pub const fn reason(self) -> HttpIpv6SourcePrefixErrorReason {
        self.reason
    }
}

impl HttpRateLimitTierPolicy {
    /// Constructs a tier policy.
    pub fn new(
        refill_tokens_per_second: u32,
        burst_tokens: u32,
        max_distinct_sources: usize,
    ) -> Result<Self, HttpRateLimitTierPolicyError> {
        if refill_tokens_per_second == 0 {
            return Err(HttpRateLimitTierPolicyError::new(
                HttpRateLimitTierPolicyErrorReason::ZeroRefillTokens,
            ));
        }
        if burst_tokens == 0 {
            return Err(HttpRateLimitTierPolicyError::new(
                HttpRateLimitTierPolicyErrorReason::ZeroBurstTokens,
            ));
        }
        if max_distinct_sources == 0 {
            return Err(HttpRateLimitTierPolicyError::new(
                HttpRateLimitTierPolicyErrorReason::ZeroMaxDistinctSources,
            ));
        }
        if refill_tokens_per_second > burst_tokens {
            return Err(HttpRateLimitTierPolicyError::new(
                HttpRateLimitTierPolicyErrorReason::RefillExceedsBurst,
            ));
        }

        Ok(Self {
            refill_tokens_per_second,
            burst_tokens,
            max_distinct_sources,
            scope: HttpRateLimitScope::PerSource,
            ipv6_source_prefix_len: 64,
        })
    }

    /// Returns a copy of this policy with explicit bucket scope.
    pub const fn with_scope(mut self, scope: HttpRateLimitScope) -> Self {
        self.scope = scope;
        self
    }

    /// Sets the IPv6 network prefix used to group per-source request buckets.
    pub fn with_ipv6_source_prefix_len(
        mut self,
        prefix_len: u8,
    ) -> Result<Self, HttpIpv6SourcePrefixError> {
        if !(48..=128).contains(&prefix_len) {
            return Err(HttpIpv6SourcePrefixError {
                reason: HttpIpv6SourcePrefixErrorReason::OutsideSupportedRange,
            });
        }
        self.ipv6_source_prefix_len = prefix_len;
        Ok(self)
    }

    /// Returns how many tokens the bucket refills each second.
    pub const fn refill_tokens_per_second(self) -> u32 {
        self.refill_tokens_per_second
    }

    /// Returns the maximum burst token capacity.
    pub const fn burst_tokens(self) -> u32 {
        self.burst_tokens
    }

    /// Returns the maximum number of distinct source buckets retained.
    pub const fn max_distinct_sources(self) -> usize {
        self.max_distinct_sources
    }

    /// Returns whether the tier keeps per-source or shared buckets.
    pub const fn scope(self) -> HttpRateLimitScope {
        self.scope
    }

    /// Returns the IPv6 prefix used for per-source request bucket identity.
    pub const fn ipv6_source_prefix_len(self) -> u8 {
        self.ipv6_source_prefix_len
    }
}

impl HttpServerSpec {
    /// Creates an HTTP server spec from validated config and app/server-owned routes.
    pub fn new(config: HttpServerConfig, app_router: Router) -> Self {
        Self {
            name: HttpListenerName::http_default(),
            visibility: HttpListenerVisibility::Public,
            config,
            app_router,
            route_visibility_policy: HttpRouteVisibilityPolicy::allow_all(),
            rate_limit_tier: None,
            rate_limit_policies: Arc::new(Vec::new()),
        }
    }

    /// Creates an HTTP server spec for a named listener.
    pub fn with_listener(
        name: HttpListenerName,
        visibility: HttpListenerVisibility,
        config: HttpServerConfig,
        app_router: Router,
    ) -> Self {
        Self {
            name,
            visibility,
            config,
            app_router,
            route_visibility_policy: HttpRouteVisibilityPolicy::allow_all(),
            rate_limit_tier: None,
            rate_limit_policies: Arc::new(Vec::new()),
        }
    }

    /// Attaches route visibility policy evaluated before app handlers run.
    pub fn with_route_visibility_policy(mut self, policy: HttpRouteVisibilityPolicy) -> Self {
        self.route_visibility_policy = policy;
        self
    }

    /// Attaches an optional listener-level rate-limit tier name.
    pub fn with_rate_limit_tier(mut self, rate_limit_tier: Option<HttpRateLimitTierName>) -> Self {
        self.rate_limit_tier = rate_limit_tier;
        self
    }

    /// Attaches named token-bucket rate-limit policies for route-level tiers.
    pub fn with_rate_limit_policies(
        mut self,
        rate_limit_policies: Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>>,
    ) -> Self {
        self.rate_limit_policies = rate_limit_policies;
        self
    }

    pub(crate) fn name(&self) -> &HttpListenerName {
        &self.name
    }

    pub(crate) fn visibility(&self) -> HttpListenerVisibility {
        self.visibility.clone()
    }

    pub(crate) fn config(&self) -> &HttpServerConfig {
        &self.config
    }

    pub(crate) fn route_visibility_policy(&self) -> &HttpRouteVisibilityPolicy {
        &self.route_visibility_policy
    }

    pub(crate) fn rate_limit_tier(&self) -> Option<&HttpRateLimitTierName> {
        self.rate_limit_tier.as_ref()
    }

    pub(crate) fn rate_limit_policies(
        &self,
    ) -> Arc<Vec<(HttpRateLimitTierName, HttpRateLimitTierPolicy)>> {
        Arc::clone(&self.rate_limit_policies)
    }

    pub(crate) fn into_router(self) -> Router {
        self.app_router
    }

    pub(crate) fn router(&self) -> &Router {
        &self.app_router
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
