// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Router;
use std::sync::Arc;
use thiserror::Error;
use tokio::net::TcpListener;
use tracing::debug;

use crate::config::HttpServerConfig;
use crate::http::{
    HttpListenerName, HttpListenerVisibility, HttpRateLimitTierName, HttpRouteVisibilityPolicy,
};
use crate::task::{ShutdownToken, TaskExecutionError, TaskExecutionErrorKind};

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
}

/// Low-cardinality reason a rate-limit tier policy was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRateLimitTierPolicyErrorReason {
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

impl HttpRateLimitTierPolicy {
    /// Constructs a tier policy.
    pub fn new(
        refill_tokens_per_second: u32,
        burst_tokens: u32,
        max_distinct_sources: usize,
    ) -> Result<Self, HttpRateLimitTierPolicyError> {
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
        })
    }

    /// Returns a copy of this policy with explicit bucket scope.
    pub const fn with_scope(mut self, scope: HttpRateLimitScope) -> Self {
        self.scope = scope;
        self
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
}

pub(crate) async fn serve_http(
    listener: TcpListener,
    router: Router,
    listener_name: HttpListenerName,
    mut shutdown: ShutdownToken,
) -> Result<(), TaskExecutionError> {
    let listener_name = listener_name.as_str().to_owned();
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let reason = shutdown.cancelled().await;
        debug!(listener_name, ?reason, "http_listener_draining");
    })
    .await
    .map_err(|_| TaskExecutionError::new(TaskExecutionErrorKind::Internal))
}

#[cfg(test)]
mod tests {
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
}
