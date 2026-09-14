// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use metrics::{Unit, describe_counter, describe_gauge, describe_histogram};

use super::names::MetricName;

pub(super) fn describe_standard_metrics() {
    describe_counter!(
        MetricName::HttpRequestCount.as_str(),
        "Total completed HTTP requests by method, route template, and status class."
    );
    describe_histogram!(
        MetricName::HttpRequestDurationSeconds.as_str(),
        Unit::Seconds,
        "HTTP request duration in seconds by method, route template, and status class."
    );
    describe_counter!(
        MetricName::HttpErrorCount.as_str(),
        "Total HTTP requests treated as errors by method, route template, and status class."
    );
    describe_counter!(
        MetricName::HttpRejectedCount.as_str(),
        "Total HTTP requests rejected by runtime backpressure before app handling."
    );
    describe_counter!(
        MetricName::RuntimeQueueSaturationCount.as_str(),
        "Total bounded runtime queue saturation events by stable queue name."
    );
    describe_counter!(
        MetricName::RuntimeAppStartupFailureCount.as_str(),
        "Total runtime app startup-check failures by stable outcome."
    );
    describe_counter!(
        MetricName::RuntimeAppCleanupFailureCount.as_str(),
        "Total runtime app cleanup failures by stable outcome."
    );
    describe_gauge!(
        MetricName::StartupInfo.as_str(),
        "Static startup/build information for the running service instance."
    );
    describe_gauge!(
        MetricName::ReadinessState.as_str(),
        "Current readiness state of the service where 1 means ready and 0 means not ready."
    );
    describe_gauge!(
        MetricName::RuntimePhase.as_str(),
        "Current server runtime lifecycle phase encoded as a stable low-cardinality numeric value."
    );
    describe_counter!(
        MetricName::ServiceDiscoveryResolveCount.as_str(),
        "Total service discovery resolver attempts by source and outcome."
    );
    describe_gauge!(
        MetricName::RateLimitBucketsLive.as_str(),
        "Current number of live in-memory rate-limit buckets for a listener."
    );
    describe_counter!(
        MetricName::RateLimitMutexPoisoned.as_str(),
        "Total rate-limit registry mutex poison recoveries."
    );
}
