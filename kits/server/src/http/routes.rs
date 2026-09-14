// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Json;
use axum::Router;
use axum::http::{StatusCode, header};
use axum::routing::{MethodRouter, get};

use crate::config::{HttpRequestLoggingConfig, HttpServerConfig};
use crate::health::{Readiness, liveness_check, readiness_check};
use crate::observability::MetricsExporter;
use crate::version::{BuildInfo, VersionResponse};

use super::content_type::PROMETHEUS_TEXT_CONTENT_TYPE;
use super::cors::cors_layer;
use super::layers::{
    body_limit_layer, concurrency_limit_layer, default_body_limit,
    normalize_http_error_responses_layer, request_id_layer, security_layer, timeout_layer,
    trace_id_layer, trace_layer,
};

/// Canonical liveness path.
pub const HEALTHZ_PATH: &str = "/healthz";
/// Canonical readiness path.
pub const READYZ_PATH: &str = "/readyz";
/// Canonical version path.
pub const VERSION_PATH: &str = "/version";
/// Canonical Prometheus metrics path.
pub const METRICS_PATH: &str = "/metrics";

/// Returns the reusable `/healthz` route.
pub fn healthz_route() -> MethodRouter {
    get(|| async {
        let response = liveness_check();
        (response.http_status_code(), Json(response))
    })
}

/// Returns the reusable `/readyz` route.
pub fn readyz_route(readiness: Readiness) -> MethodRouter {
    get(move || {
        let readiness = readiness.clone();

        async move {
            let response = readiness_check(&readiness);
            (response.http_status_code(), Json(response))
        }
    })
}

/// Returns the reusable `/version` route.
///
/// This route is intended for safe build/version metadata only. The returned
/// `BuildInfo` contract must never grow to include secrets, credentials, raw
/// configuration values, or other sensitive operational state.
pub fn version_route(build_info: BuildInfo) -> MethodRouter {
    get(move || {
        let build_info = build_info.clone();
        async move { (StatusCode::OK, Json(VersionResponse::from(build_info))) }
    })
}

/// Returns the reusable `/metrics` route.
pub fn metrics_route(exporter: MetricsExporter) -> MethodRouter {
    get(move || {
        let exporter = exporter.clone();
        async move {
            (
                [(header::CONTENT_TYPE, PROMETHEUS_TEXT_CONTENT_TYPE)],
                exporter.render(),
            )
        }
    })
}

/// Returns a router that exposes the standard operational HTTP routes.
pub fn operational_routes(
    readiness: Readiness,
    build_info: BuildInfo,
    exporter: MetricsExporter,
) -> Router {
    Router::new()
        .route(HEALTHZ_PATH, healthz_route())
        .route(READYZ_PATH, readyz_route(readiness))
        .route(VERSION_PATH, version_route(build_info))
        .route(METRICS_PATH, metrics_route(exporter))
}

/// Applies the standard HTTP router layers used across services.
pub fn apply_standard_router_layers<S>(router: Router<S>, config: &HttpServerConfig) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    apply_standard_router_layers_with_request_logging(
        router,
        config,
        HttpRequestLoggingConfig::defaults(),
    )
}

/// Applies the standard HTTP router layers used across services with explicit
/// request-completion logging behavior.
pub fn apply_standard_router_layers_with_request_logging<S>(
    router: Router<S>,
    config: &HttpServerConfig,
    request_logging: HttpRequestLoggingConfig,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router
        .layer(trace_layer(request_logging))
        .layer(body_limit_layer(config))
        .layer(normalize_http_error_responses_layer())
        .layer(default_body_limit(config))
        .layer(timeout_layer(config))
        .layer(concurrency_limit_layer(config))
        .layer(security_layer(config.security()))
        .layer(trace_id_layer())
        .layer(request_id_layer())
        .layer(cors_layer(config))
}

#[cfg(test)]
mod tests;
