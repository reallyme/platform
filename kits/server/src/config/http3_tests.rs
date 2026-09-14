// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::Http3ServerConfig;
use crate::config::{ConfigError, ConfigValidationErrorReason, Http3ServerConfigField};

#[test]
fn http3_is_disabled_by_default() {
    assert!(!Http3ServerConfig::default().enabled());
    assert!(!Http3ServerConfig::disabled().enabled());
}

#[test]
fn http3_enable_fails_closed_until_tls_quic_listener_exists() {
    assert_eq!(
        Http3ServerConfig::enable(),
        Err(ConfigError::InvalidHttp3ServerConfig {
            field: Http3ServerConfigField::Enabled,
            reason: ConfigValidationErrorReason::UnsupportedProtocol,
        })
    );
}
