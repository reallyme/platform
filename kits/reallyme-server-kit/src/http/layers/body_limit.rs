// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::extract::DefaultBodyLimit;
use tower_http::limit::RequestBodyLimitLayer;

use crate::config::HttpServerConfig;

/// Creates a hard request body limit layer.
pub fn body_limit_layer(config: &HttpServerConfig) -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(config.body_limit().request_body_limit().as_usize())
}

/// Creates Axum's default body limit extractor setting.
pub fn default_body_limit(config: &HttpServerConfig) -> DefaultBodyLimit {
    DefaultBodyLimit::max(config.body_limit().request_body_limit().as_usize())
}
