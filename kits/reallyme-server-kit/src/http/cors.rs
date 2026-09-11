// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::{HeaderValue, Method, header};
use reallyme_app_kit::AppCorsConfig;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::config::{CorsConfig, HttpServerConfig};

use super::ids::{X_REQUEST_ID, X_TRACE_ID};

/// Creates the shared CORS layer for a service listener.
pub fn cors_layer(config: &HttpServerConfig) -> CorsLayer {
    let layer = base_cors_layer();

    match config.cors() {
        CorsConfig::NoCors => layer,
        CorsConfig::ExactOrigins(origins) => layer.allow_origin(AllowOrigin::list(
            origins
                .as_slice()
                .iter()
                .map(|origin| origin.as_header_value().clone()),
        )),
        CorsConfig::AnyForDevelopmentOnly => layer.allow_origin(AllowOrigin::any()),
    }
}

/// Creates a per-app CORS layer from a host-neutral [`AppCorsConfig`].
///
/// Returns `None` when the app has not declared any allowed origins, so callers
/// can skip layering entirely instead of attaching a no-op layer that would
/// still emit preflight responses.
pub fn app_cors_layer(config: &AppCorsConfig) -> Option<CorsLayer> {
    if !config.is_enabled() {
        return None;
    }

    let origins: Vec<HeaderValue> = config
        .allowed_origins()
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin.as_str()).ok())
        .collect();

    if origins.is_empty() {
        return None;
    }

    Some(base_cors_layer().allow_origin(AllowOrigin::list(origins)))
}

fn base_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            X_REQUEST_ID,
            X_TRACE_ID,
        ])
        .expose_headers([X_REQUEST_ID, X_TRACE_ID])
}

#[cfg(test)]
mod tests {
    use super::app_cors_layer;
    use crate::config::CorsConfig;
    use reallyme_app_kit::AppJsoncConfigDocument;

    #[test]
    fn no_cors_is_the_default_safe_posture() {
        assert!(matches!(CorsConfig::no_cors(), CorsConfig::NoCors));
    }

    #[test]
    fn app_cors_layer_is_none_when_no_origins_declared() {
        let document =
            AppJsoncConfigDocument::<reallyme_app_kit::NoAppCustomConfig>::from_jsonc_str(
                r#"{
                "cors": {"allowed_origins": []},
                "reflection_enabled": false,
                "downstream": {}
            }"#,
            )
            .expect("valid empty-cors fixture");
        assert!(app_cors_layer(document.cors()).is_none());
    }

    #[test]
    fn app_cors_layer_is_some_when_origins_declared() {
        let document =
            AppJsoncConfigDocument::<reallyme_app_kit::NoAppCustomConfig>::from_jsonc_str(
                r#"{
                "cors": {"allowed_origins": ["https://app.reallyme.net"]},
                "reflection_enabled": false,
                "downstream": {}
            }"#,
            )
            .expect("valid single-origin fixture");
        assert!(app_cors_layer(document.cors()).is_some());
    }
}
