// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! HTTP runtime configuration re-exports.
//!
//! The validated runtime configuration types are owned by `crate::config` so
//! they can be shared across transports and startup logic. They are re-exported
//! here for HTTP-focused consumers that prefer `crate::http::*` imports.

pub use crate::config::{
    BodyLimitConfig, CorsConfig, ExactCorsOrigin, HttpServerConfig, RequestBodyLimitBytes,
    RequestTimeout, TimeoutConfig,
};

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
