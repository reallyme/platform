// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::FromStr;
use std::time::Duration;

use crate::config::{
    BindAddress, BodyLimitConfigField, ConfigError, ConfigValidationErrorReason, CorsConfig,
    DEFAULT_METRICS_IDLE_TIMEOUT, Http3ServerConfig, Http3ServerConfigField, LogFormat,
    LogFormatParseError, MetricsIdleTimeout, ObservabilityConfig, ObservabilityConfigField,
    RequestBodyLimitBytes, RequestTimeout, SecretString, ServiceEnvironment,
    ServiceEnvironmentParseError, TimeoutConfigField,
};
#[cfg(feature = "tonic-grpc")]
use crate::config::{BodyLimitConfig, GrpcServerConfig, HttpServerConfig, TimeoutConfig};

#[test]
fn config_rejects_invalid_timeout_body_limit_values() {
    let timeout_error = RequestTimeout::new(Duration::ZERO);
    let body_limit_error = RequestBodyLimitBytes::new(0);

    assert_eq!(
        timeout_error,
        Err(ConfigError::InvalidTimeoutConfig {
            field: TimeoutConfigField::RequestTimeout,
            reason: ConfigValidationErrorReason::MustBeGreaterThanZero,
        })
    );
    assert_eq!(
        body_limit_error,
        Err(ConfigError::InvalidBodyLimitConfig {
            field: BodyLimitConfigField::RequestBodyLimitBytes,
            reason: ConfigValidationErrorReason::MustBeGreaterThanZero,
        })
    );
}

#[test]
fn secrets_are_not_printed_in_debug_output() {
    let secret = SecretString::new("top-secret-value".to_owned());
    let rendered = format!("{secret:?}");

    assert_eq!(rendered, "Secret([REDACTED])");
    assert!(!rendered.contains("top-secret-value"));
}

#[test]
fn startup_config_summary_redacts_secret_material() {
    let summary = format!("{:?}", SecretString::new("redact-me".to_owned()));

    assert_eq!(summary, "Secret([REDACTED])");
    assert!(!summary.contains("redact-me"));
}

#[test]
#[cfg(feature = "tonic-grpc")]
fn server_config_summaries_expose_inspectable_values() {
    let socket_address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8443));
    let bind_address = BindAddress::new(socket_address).expect("valid test bind address");
    let timeout = RequestTimeout::new(Duration::from_secs(5)).expect("valid test timeout");
    let body_limit = RequestBodyLimitBytes::new(4096).expect("valid test body limit");

    let http_config = HttpServerConfig::new(
        bind_address,
        CorsConfig::no_cors(),
        TimeoutConfig::new(timeout),
        BodyLimitConfig::new(body_limit),
    );
    let grpc_config =
        GrpcServerConfig::new(BindAddress::new(socket_address).expect("valid grpc bind address"));

    let http_summary = http_config.startup_summary();
    let grpc_summary = grpc_config.startup_summary();

    assert_eq!(http_summary.bind_address().port().as_u16(), 8443);
    assert_eq!(
        http_summary.timeout().request_timeout().as_duration(),
        Duration::from_secs(5)
    );
    assert_eq!(
        http_summary.body_limit().request_body_limit().as_usize(),
        4096
    );
    assert!(matches!(http_summary.cors(), CorsConfig::NoCors));
    assert!(!http_summary.http3().enabled());
    assert_eq!(grpc_summary.bind_address().port().as_u16(), 8443);
}

#[test]
fn http3_quic_enable_fails_closed_until_tls_runtime_support_exists() {
    assert_eq!(
        Http3ServerConfig::enable(),
        Err(ConfigError::InvalidHttp3ServerConfig {
            field: Http3ServerConfigField::Enabled,
            reason: ConfigValidationErrorReason::UnsupportedProtocol,
        })
    );
}

#[test]
fn observability_config_rejects_empty_filter_directives() {
    let idle_timeout =
        MetricsIdleTimeout::new(Duration::from_secs(30)).expect("valid idle timeout fixture");

    let result = ObservabilityConfig::new(
        ServiceEnvironment::Local,
        LogFormat::PlainText,
        false,
        String::new(),
        idle_timeout,
    );

    assert_eq!(
        result,
        Err(ConfigError::InvalidObservabilityConfig {
            field: ObservabilityConfigField::TracingFilterDirectives,
            reason: ConfigValidationErrorReason::MustBeNonEmpty,
        })
    );
}

#[test]
fn observability_startup_summary_exposes_inspectable_values() {
    let idle_timeout =
        MetricsIdleTimeout::new(Duration::from_secs(30)).expect("valid idle timeout fixture");
    let config = ObservabilityConfig::new(
        ServiceEnvironment::Staging,
        LogFormat::Json,
        true,
        "info,reallyme_server_kit=debug".to_owned(),
        idle_timeout,
    )
    .expect("valid observability config fixture");

    let summary = config.startup_summary();

    assert_eq!(summary.service_environment(), ServiceEnvironment::Staging);
    assert_eq!(summary.log_format(), LogFormat::Json);
    assert!(summary.emit_span_events());
    assert!(summary.tracing_filter_configured());
    assert!(!format!("{summary:?}").contains("info,reallyme_server_kit=debug"));
    assert_eq!(
        summary.metrics_idle_timeout().as_duration(),
        Duration::from_secs(30)
    );
}

#[test]
fn metrics_idle_timeout_rejects_too_small_values() {
    let result = MetricsIdleTimeout::new(Duration::from_secs(5));

    assert_eq!(
        result,
        Err(ConfigError::InvalidObservabilityConfig {
            field: ObservabilityConfigField::MetricsIdleTimeout,
            reason: ConfigValidationErrorReason::MustBeAtLeastMinimum,
        })
    );
}

#[test]
fn typed_bind_addresses_ports_and_durations_are_preserved() {
    let bind_address =
        BindAddress::new(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8443)))
            .expect("valid test bind address");
    let timeout = RequestTimeout::new(Duration::from_secs(5)).expect("valid test timeout");
    let body_limit = RequestBodyLimitBytes::new(4096).expect("valid test body limit");

    assert_eq!(bind_address.ip_addr(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(bind_address.port().as_u16(), 8443);
    assert_eq!(timeout.as_duration(), Duration::from_secs(5));
    assert_eq!(body_limit.as_usize(), 4096);
}

#[test]
fn log_format_and_service_environment_parse_supported_values() {
    assert_eq!(LogFormat::from_str("json"), Ok(LogFormat::Json));
    assert_eq!(LogFormat::from_str("JSON"), Ok(LogFormat::Json));
    assert_eq!(LogFormat::from_str("plain_text"), Ok(LogFormat::PlainText));
    assert_eq!(LogFormat::from_str("PLAIN_TEXT"), Ok(LogFormat::PlainText));
    assert_eq!(LogFormat::from_str("pretty"), Ok(LogFormat::PlainText));
    assert_eq!(LogFormat::from_str("PRETTY"), Ok(LogFormat::PlainText));
    assert_eq!(
        ServiceEnvironment::from_str("staging"),
        Ok(ServiceEnvironment::Staging)
    );
    assert_eq!(
        ServiceEnvironment::from_str("PROD"),
        Ok(ServiceEnvironment::Prod)
    );
}

#[test]
fn log_format_and_service_environment_reject_unsupported_values_with_typed_errors() {
    assert_eq!(
        LogFormat::from_str("console"),
        Err(LogFormatParseError::UnsupportedValue)
    );
    assert_eq!(
        ServiceEnvironment::from_str("qa"),
        Err(ServiceEnvironmentParseError::UnsupportedValue)
    );
}

#[test]
fn cors_config_accepts_exact_origin() {
    let result = CorsConfig::allow_exact_origin("https://reallyme.example");

    assert!(
        result.is_ok(),
        "expected exact-origin fixture to validate successfully"
    );
}

#[test]
fn default_metrics_idle_timeout_remains_valid() {
    let result = MetricsIdleTimeout::new(DEFAULT_METRICS_IDLE_TIMEOUT);

    assert!(
        result.is_ok(),
        "server-kit default metrics idle timeout must remain valid"
    );
}
