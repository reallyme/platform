// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use super::{RequestBodyLimitBytes, RequestTimeout};
    use std::time::Duration;

    #[test]
    fn typed_timeout_and_body_limit_validation_remain_in_config() {
        assert!(RequestTimeout::new(Duration::ZERO).is_err());
        assert!(RequestBodyLimitBytes::new(0).is_err());
    }
}
