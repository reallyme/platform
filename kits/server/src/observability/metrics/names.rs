// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Stable observability metric names used by the server kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricName {
    /// Total completed HTTP requests.
    HttpRequestCount,
    /// HTTP request duration in seconds.
    HttpRequestDurationSeconds,
    /// Total HTTP requests treated as errors.
    HttpErrorCount,
    /// Total HTTP requests rejected before handler execution.
    HttpRejectedCount,
    /// Total HTTP per-tier rate-limit decisions.
    HttpRateLimitDecisionCount,
    /// Total bounded runtime queue saturation events.
    RuntimeQueueSaturationCount,
    /// Total runtime app startup-check failures.
    RuntimeAppStartupFailureCount,
    /// Total runtime app cleanup failures or timeouts.
    RuntimeAppCleanupFailureCount,
    /// Static startup/build information gauge.
    StartupInfo,
    /// Current readiness state gauge.
    ReadinessState,
    /// Current runtime lifecycle phase gauge.
    RuntimePhase,
    /// Total service discovery resolution attempts.
    ServiceDiscoveryResolveCount,
    /// Live in-memory rate-limit buckets by listener.
    RateLimitBucketsLive,
    /// Total rate-limit registry mutex poison recoveries.
    RateLimitMutexPoisoned,
}

impl MetricName {
    /// Returns the stable metric name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HttpRequestCount => "reallyme_http_requests_total",
            Self::HttpRequestDurationSeconds => "reallyme_http_request_duration_seconds",
            Self::HttpErrorCount => "reallyme_http_errors_total",
            Self::HttpRejectedCount => "reallyme_http_requests_rejected_total",
            Self::HttpRateLimitDecisionCount => "reallyme_http_rate_limit_decisions_total",
            Self::RuntimeQueueSaturationCount => "reallyme_runtime_queue_saturation_total",
            Self::RuntimeAppStartupFailureCount => "reallyme_runtime_app_startup_failures_total",
            Self::RuntimeAppCleanupFailureCount => "reallyme_runtime_app_cleanup_failures_total",
            Self::StartupInfo => "reallyme_service_startup_info",
            Self::ReadinessState => "reallyme_service_readiness_state",
            Self::RuntimePhase => "reallyme_service_runtime_phase",
            Self::ServiceDiscoveryResolveCount => "reallyme_service_discovery_resolve_total",
            Self::RateLimitBucketsLive => "reallyme_rate_limit_buckets_live",
            Self::RateLimitMutexPoisoned => "reallyme_rate_limit_mutex_poisoned_total",
        }
    }
}
