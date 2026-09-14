// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Observability bootstrap failures.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ObservabilityError {
    /// The configured tracing directive string could not be parsed.
    #[error("invalid tracing filter configuration")]
    InvalidTracingFilter,
    /// Global tracing has already been installed for this process.
    #[error("tracing subscriber has already been initialized")]
    TracingAlreadyInitialized,
    /// Global metrics recording has already been installed for this process.
    #[error("prometheus recorder has already been installed")]
    MetricsRecorderAlreadyInstalled,
}

/// Metric-label validation failures.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum MetricLabelError {
    /// The route template label was empty.
    #[error("route template label cannot be empty")]
    EmptyRouteTemplate,
    /// The route template label did not use an absolute route-template shape.
    #[error("route template label must begin with '/'")]
    RouteTemplateMustStartWithSlash,
    /// The route template label contained a query string marker.
    #[error("route template label must not contain a query string")]
    RouteTemplateMustNotContainQueryString,
    /// The route template label contained a full URL scheme marker.
    #[error("route template label must not contain a URL scheme")]
    RouteTemplateMustNotContainUrlScheme,
    /// The route template label contained whitespace.
    #[error("route template label must not contain whitespace")]
    RouteTemplateMustNotContainWhitespace,
}
