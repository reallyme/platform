// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::str::FromStr;
use std::time::Duration;

use thiserror::Error;

use super::environment::ServiceEnvironment;
use super::error::{ConfigError, ConfigValidationErrorReason, ObservabilityConfigField};
use super::timing::MetricsIdleTimeout;

/// Supported logging formats for server-process observability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Structured JSON logs intended for production ingestion.
    Json,
    /// Human-readable logs intended for local development. Config loaders may
    /// accept `pretty` as an operator-friendly alias for this mode.
    PlainText,
}

/// Failure to parse an observability log format token.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum LogFormatParseError {
    /// The provided value does not match a supported log format token.
    #[error("log format value is unsupported")]
    UnsupportedValue,
}

/// Supported HTTP request-completion logging modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestLogMode {
    /// Never emit per-request completion logs.
    Disabled,
    /// Emit completion logs only for HTTP error responses.
    ErrorsOnly,
    /// Emit completion logs for sampled successful requests and all error/slow requests.
    Sampled,
    /// Emit completion logs for every request.
    All,
}

/// Additional structured HTTP access-log fields that operators may opt into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestLogField {
    /// `listener.name`
    ListenerName,
    /// Normalized external host, never a raw forwarded header value.
    ExternalHost,
    /// Normalized external scheme/proto, never a raw forwarded header value.
    ExternalProto,
    /// Normalized external port, when one was present.
    ExternalPort,
    /// Safely-derived normalized external origin/base URL.
    ExternalOrigin,
    /// Normalized client IP selected by trusted proxy policy.
    NormalizedClientIp,
}

impl HttpRequestLogField {
    /// Returns the stable config token for this field.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ListenerName => "listener_name",
            Self::ExternalHost => "external_host",
            Self::ExternalProto => "external_proto",
            Self::ExternalPort => "external_port",
            Self::ExternalOrigin => "external_origin",
            Self::NormalizedClientIp => "normalized_client_ip",
        }
    }
}

/// Failure to parse an HTTP request-log field token.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestLogFieldParseError {
    /// The provided value does not match a supported field token.
    #[error("http request log field value is unsupported")]
    UnsupportedValue,
}

impl FromStr for HttpRequestLogField {
    type Err = HttpRequestLogFieldParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "listener_name" => Ok(Self::ListenerName),
            "external_host" => Ok(Self::ExternalHost),
            "external_proto" => Ok(Self::ExternalProto),
            "external_port" => Ok(Self::ExternalPort),
            "external_origin" => Ok(Self::ExternalOrigin),
            "normalized_client_ip" => Ok(Self::NormalizedClientIp),
            _ => Err(HttpRequestLogFieldParseError::UnsupportedValue),
        }
    }
}

/// Bounded opt-in structured HTTP access-log field selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpRequestLogFields {
    listener_name: bool,
    external_host: bool,
    external_proto: bool,
    external_port: bool,
    external_origin: bool,
    normalized_client_ip: bool,
}

impl HttpRequestLogFields {
    /// Returns the fail-closed default selection.
    pub const fn defaults() -> Self {
        Self {
            listener_name: false,
            external_host: false,
            external_proto: false,
            external_port: false,
            external_origin: false,
            normalized_client_ip: false,
        }
    }

    /// Returns a copy with the selected field enabled.
    pub const fn with_field(mut self, field: HttpRequestLogField) -> Self {
        match field {
            HttpRequestLogField::ListenerName => self.listener_name = true,
            HttpRequestLogField::ExternalHost => self.external_host = true,
            HttpRequestLogField::ExternalProto => self.external_proto = true,
            HttpRequestLogField::ExternalPort => self.external_port = true,
            HttpRequestLogField::ExternalOrigin => self.external_origin = true,
            HttpRequestLogField::NormalizedClientIp => self.normalized_client_ip = true,
        }

        self
    }

    /// Returns whether `listener.name` is enabled.
    pub const fn listener_name(self) -> bool {
        self.listener_name
    }

    /// Returns whether normalized external host logging is enabled.
    pub const fn external_host(self) -> bool {
        self.external_host
    }

    /// Returns whether normalized external proto logging is enabled.
    pub const fn external_proto(self) -> bool {
        self.external_proto
    }

    /// Returns whether normalized external port logging is enabled.
    pub const fn external_port(self) -> bool {
        self.external_port
    }

    /// Returns whether normalized external origin logging is enabled.
    pub const fn external_origin(self) -> bool {
        self.external_origin
    }

    /// Returns whether normalized client IP logging is enabled.
    pub const fn normalized_client_ip(self) -> bool {
        self.normalized_client_ip
    }
}

/// Failure to parse an HTTP request-log mode token.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestLogModeParseError {
    /// The provided value does not match a supported request-log mode token.
    #[error("http request log mode value is unsupported")]
    UnsupportedValue,
}

impl FromStr for HttpRequestLogMode {
    type Err = HttpRequestLogModeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "disabled" => Ok(Self::Disabled),
            "errors_only" => Ok(Self::ErrorsOnly),
            "sampled" => Ok(Self::Sampled),
            "all" => Ok(Self::All),
            _ => Err(HttpRequestLogModeParseError::UnsupportedValue),
        }
    }
}

/// Typed HTTP request logging configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HttpRequestLoggingConfig {
    mode: HttpRequestLogMode,
    sample_rate: f64,
    slow_request_threshold: Option<Duration>,
    fields: HttpRequestLogFields,
}

impl HttpRequestLoggingConfig {
    /// Returns the default HTTP request logging profile.
    pub const fn defaults() -> Self {
        Self {
            mode: HttpRequestLogMode::All,
            sample_rate: 1.0,
            slow_request_threshold: None,
            fields: HttpRequestLogFields::defaults(),
        }
    }

    /// Constructs validated HTTP request logging configuration.
    pub fn new(
        mode: HttpRequestLogMode,
        sample_rate: Option<f64>,
        slow_request_threshold: Option<Duration>,
    ) -> Result<Self, ConfigError> {
        let sample_rate = sample_rate.unwrap_or(1.0);
        if !(0.0..=1.0).contains(&sample_rate) {
            return Err(ConfigError::InvalidObservabilityConfig {
                field: ObservabilityConfigField::HttpRequestLogSampleRate,
                reason: if sample_rate < 0.0 {
                    ConfigValidationErrorReason::MustBeAtLeastMinimum
                } else {
                    ConfigValidationErrorReason::MustBeLessThanOrEqualToMaximum
                },
            });
        }

        if slow_request_threshold.is_some_and(|threshold| threshold.is_zero()) {
            return Err(ConfigError::InvalidObservabilityConfig {
                field: ObservabilityConfigField::HttpSlowRequestThreshold,
                reason: ConfigValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        Ok(Self {
            mode,
            sample_rate,
            slow_request_threshold,
            fields: HttpRequestLogFields::defaults(),
        })
    }

    /// Returns a copy with explicit structured access-log field selection.
    pub const fn with_fields(mut self, fields: HttpRequestLogFields) -> Self {
        self.fields = fields;
        self
    }

    /// Returns the configured request-log mode.
    pub fn mode(&self) -> HttpRequestLogMode {
        self.mode
    }

    /// Returns the configured sampling rate in the inclusive range `0.0..=1.0`.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Returns the optional slow-request threshold.
    pub fn slow_request_threshold(&self) -> Option<Duration> {
        self.slow_request_threshold
    }

    /// Returns the structured access-log field selection.
    pub fn fields(&self) -> HttpRequestLogFields {
        self.fields
    }
}

impl FromStr for LogFormat {
    type Err = LogFormatParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // Log format values are parsed case-insensitively so deployment
        // tooling can provide conventional uppercase env-style values without
        // forcing each consuming service to normalize them first.
        if value.eq_ignore_ascii_case("json") {
            Ok(Self::Json)
        } else if value.eq_ignore_ascii_case("plain_text") || value.eq_ignore_ascii_case("pretty") {
            Ok(Self::PlainText)
        } else {
            Err(LogFormatParseError::UnsupportedValue)
        }
    }
}

/// Shared observability configuration.
///
/// This type must remain safe to include in startup summaries and config-derived
/// logs. It should never contain secrets, credentials, or tokens. The raw
/// tracing filter remains available to tracing initialization, but summary
/// types expose only whether it was configured so arbitrary deployment-provided
/// directives are not echoed into logs.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservabilityConfig {
    service_environment: ServiceEnvironment,
    log_format: LogFormat,
    emit_span_events: bool,
    tracing_filter_directives: String,
    metrics_idle_timeout: MetricsIdleTimeout,
    request_logging: HttpRequestLoggingConfig,
}

impl ObservabilityConfig {
    /// Constructs validated observability configuration.
    ///
    /// The tracing filter directives belong here because they configure the
    /// runtime logging/telemetry system itself. Service-specific environment
    /// variable names used to load them do not belong here.
    pub fn new(
        service_environment: ServiceEnvironment,
        log_format: LogFormat,
        emit_span_events: bool,
        tracing_filter_directives: String,
        metrics_idle_timeout: MetricsIdleTimeout,
    ) -> Result<Self, ConfigError> {
        if tracing_filter_directives.trim().is_empty() {
            return Err(ConfigError::InvalidObservabilityConfig {
                field: ObservabilityConfigField::TracingFilterDirectives,
                reason: ConfigValidationErrorReason::MustBeNonEmpty,
            });
        }

        Ok(Self {
            service_environment,
            log_format,
            emit_span_events,
            tracing_filter_directives,
            metrics_idle_timeout,
            request_logging: HttpRequestLoggingConfig::defaults(),
        })
    }

    /// Returns a copy of this config with validated HTTP request logging settings.
    pub fn with_http_request_logging(mut self, request_logging: HttpRequestLoggingConfig) -> Self {
        self.request_logging = request_logging;
        self
    }

    /// Returns the service environment.
    pub fn service_environment(&self) -> ServiceEnvironment {
        self.service_environment
    }

    /// Returns the log format.
    pub fn log_format(&self) -> LogFormat {
        self.log_format
    }

    /// Returns whether tracing span lifecycle events should be emitted.
    pub fn emit_span_events(&self) -> bool {
        self.emit_span_events
    }

    /// Returns the tracing filter directives.
    pub fn tracing_filter_directives(&self) -> &str {
        self.tracing_filter_directives.as_str()
    }

    /// Returns the metrics idle timeout.
    pub fn metrics_idle_timeout(&self) -> MetricsIdleTimeout {
        self.metrics_idle_timeout
    }

    /// Returns HTTP request logging settings.
    pub fn request_logging(&self) -> HttpRequestLoggingConfig {
        self.request_logging
    }

    /// Returns a startup-safe configuration summary.
    pub fn startup_summary(&self) -> ObservabilityConfigSummary {
        ObservabilityConfigSummary {
            service_environment: self.service_environment,
            log_format: self.log_format,
            emit_span_events: self.emit_span_events,
            tracing_filter_configured: !self.tracing_filter_directives.trim().is_empty(),
            metrics_idle_timeout: self.metrics_idle_timeout,
            request_logging: self.request_logging,
        }
    }
}

/// Safe startup summary for observability configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservabilityConfigSummary {
    service_environment: ServiceEnvironment,
    log_format: LogFormat,
    emit_span_events: bool,
    tracing_filter_configured: bool,
    metrics_idle_timeout: MetricsIdleTimeout,
    request_logging: HttpRequestLoggingConfig,
}

impl ObservabilityConfigSummary {
    /// Returns the service environment.
    pub fn service_environment(&self) -> ServiceEnvironment {
        self.service_environment
    }

    /// Returns the log format.
    pub fn log_format(&self) -> LogFormat {
        self.log_format
    }

    /// Returns whether tracing span lifecycle events should be emitted.
    pub fn emit_span_events(&self) -> bool {
        self.emit_span_events
    }

    /// Returns whether a non-empty tracing filter was configured.
    pub fn tracing_filter_configured(&self) -> bool {
        self.tracing_filter_configured
    }

    /// Returns the metrics idle timeout.
    pub fn metrics_idle_timeout(&self) -> MetricsIdleTimeout {
        self.metrics_idle_timeout
    }

    /// Returns HTTP request logging settings.
    pub fn request_logging(&self) -> HttpRequestLoggingConfig {
        self.request_logging
    }
}
