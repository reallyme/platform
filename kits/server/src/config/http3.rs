// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::{ConfigError, ConfigValidationErrorReason, Http3ServerConfigField};

/// HTTP/3 over QUIC runtime configuration.
///
/// HTTP/3 is intentionally modeled separately from the TCP HTTP listener
/// because it uses UDP, TLS, ALPN, different load-balancer behavior, and
/// different operational firewall assumptions. The current server kit does
/// not yet own TLS certificate configuration, so public HTTP/3 cannot be
/// safely enabled in production yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Http3ServerConfig {
    enabled: bool,
}

impl Http3ServerConfig {
    /// Returns the current safe default: HTTP/3 disabled.
    pub const fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Attempts to enable HTTP/3 over QUIC.
    ///
    /// This intentionally fails closed until the server kit has a production
    /// TLS/QUIC listener implementation and certificate-loading policy. Keeping
    /// the typed config surface now prevents future JSON/env schemas from
    /// growing ad hoc booleans or stringly protocol switches.
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
