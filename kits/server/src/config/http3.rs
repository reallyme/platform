// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::{ConfigError, ConfigValidationErrorReason, Http3ServerConfigField};

/// HTTP/3 over QUIC runtime configuration.
///
/// HTTP/3 is intentionally modeled separately from the TCP HTTP listener
/// because it uses UDP, TLS, ALPN, different load-balancer behavior, and
/// different operational firewall assumptions. Server-kit has no TLS/QUIC
/// listener or certificate-loading policy. HTTP/3 is unsupported and cannot
/// be enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Http3ServerConfig {
    enabled: bool,
}

impl Http3ServerConfig {
    /// Returns HTTP/3 disabled, the only supported configuration.
    pub const fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Attempts to enable HTTP/3 over QUIC.
    ///
    /// Always returns [`ConfigValidationErrorReason::UnsupportedProtocol`]
    /// because this runtime does not implement HTTP/3.
    pub fn enable() -> Result<Self, ConfigError> {
        Err(ConfigError::InvalidHttp3ServerConfig {
            field: Http3ServerConfigField::Enabled,
            reason: ConfigValidationErrorReason::UnsupportedProtocol,
        })
    }

    /// Returns whether HTTP/3 over QUIC is enabled.
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

impl Default for Http3ServerConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

#[cfg(test)]
#[path = "http3_tests.rs"]
mod tests;
