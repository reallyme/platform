// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use reallyme_app_kit::{AppConfigProfile, parse_jsonc_config};
use reallyme_server_kit::config::{
    BindAddress, BodyLimitConfig, CorsConfig, ExternalOriginPolicyConfig, HostAuthority,
    HostAuthorityPolicy, HttpSecurityConfig, HttpServerConfig, LogFormat, MetricsIdleTimeout,
    ObservabilityConfig, OperationalRouteAccess, RequestBodyLimitBytes, RequestTimeout,
    SecurityHeadersConfig, ServiceEnvironment, TimeoutConfig, TrustedProxyHeaderFamily,
    TrustedProxyHeaders, TrustedProxyRange,
};
use reallyme_server_kit::task::ShutdownTimeout;
use serde::Deserialize;

use crate::error::{ExampleServerError, ExampleServerErrorReason};

const MAX_SERVER_CONFIG_BYTES: u64 = 1_048_576;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExampleServerConfig {
    application_profile: AppConfigProfile,
    bind_address: SocketAddr,
    request_timeout_seconds: u64,
    request_body_limit_bytes: usize,
    metrics_idle_timeout_seconds: u64,
    shutdown_timeout_seconds: u64,
    #[serde(default)]
    allowed_hosts: Vec<String>,
    #[serde(default)]
    trusted_proxy_ranges: Vec<String>,
    external_origin_policy: Option<RawExternalOriginPolicy>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExternalOriginPolicy {
    header_family: RawTrustedProxyHeaderFamily,
    trusted_forwarded_host: bool,
    trusted_forwarded_proto: bool,
    require_https_external_scheme: bool,
    strict_forwarded_header_consistency: bool,
    strip_raw_proxy_headers: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawTrustedProxyHeaderFamily {
    Forwarded,
    XForwarded,
}

pub(crate) struct ExampleServerConfig {
    application_profile: AppConfigProfile,
    http: HttpServerConfig,
    observability: ObservabilityConfig,
    shutdown_timeout: ShutdownTimeout,
}

impl ExampleServerConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, ExampleServerError> {
        let file = File::open(path).map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigFileUnreadable)
        })?;
        let read_limit = MAX_SERVER_CONFIG_BYTES.checked_add(1).ok_or_else(|| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigFileTooLarge)
        })?;
        let initial_capacity = usize::try_from(read_limit).map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigFileTooLarge)
        })?;
        let mut bytes = Vec::with_capacity(initial_capacity);
        file.take(read_limit).read_to_end(&mut bytes).map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigFileUnreadable)
        })?;

        if u64::try_from(bytes.len()).map_or(true, |length| length > MAX_SERVER_CONFIG_BYTES) {
            return Err(ExampleServerError::new(
                ExampleServerErrorReason::ServerConfigFileTooLarge,
            ));
        }

        let document = String::from_utf8(bytes).map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigFileNotUtf8)
        })?;
        Self::from_jsonc_str(document.as_str())
    }

    fn from_jsonc_str(value: &str) -> Result<Self, ExampleServerError> {
        let raw = parse_jsonc_config::<RawExampleServerConfig>(value).map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigDocumentInvalid)
        })?;
        let bind_address = BindAddress::new(raw.bind_address)
            .map_err(|_| ExampleServerError::new(ExampleServerErrorReason::BindAddressInvalid))?;
        if !bind_address.ip_addr().is_loopback() && raw.allowed_hosts.is_empty() {
            return Err(ExampleServerError::new(
                ExampleServerErrorReason::AllowedHostsInvalid,
            ));
        }
        let host_policy = if raw.allowed_hosts.is_empty() {
            HostAuthorityPolicy::allow_any()
        } else {
            let hosts = raw
                .allowed_hosts
                .into_iter()
                .map(HostAuthority::new)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| {
                    ExampleServerError::new(ExampleServerErrorReason::AllowedHostsInvalid)
                })?;
            HostAuthorityPolicy::allow_list(hosts).map_err(|_| {
                ExampleServerError::new(ExampleServerErrorReason::AllowedHostsInvalid)
            })?
        };
        let proxy_ranges = raw
            .trusted_proxy_ranges
            .iter()
            .map(|range| TrustedProxyRange::parse(range))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                ExampleServerError::new(ExampleServerErrorReason::TrustedProxyRangesInvalid)
            })?;
        let trusted_proxy_headers = if proxy_ranges.is_empty() {
            TrustedProxyHeaders::ignore_all()
        } else {
            TrustedProxyHeaders::trust_configured_proxies(proxy_ranges).map_err(|_| {
                ExampleServerError::new(ExampleServerErrorReason::TrustedProxyRangesInvalid)
            })?
        };
        let external_origin_policy = match raw.external_origin_policy {
            Some(policy) => {
                if matches!(trusted_proxy_headers, TrustedProxyHeaders::IgnoreAll) {
                    return Err(ExampleServerError::new(
                        ExampleServerErrorReason::ExternalOriginPolicyInvalid,
                    ));
                }
                let family = match policy.header_family {
                    RawTrustedProxyHeaderFamily::Forwarded => TrustedProxyHeaderFamily::Forwarded,
                    RawTrustedProxyHeaderFamily::XForwarded => TrustedProxyHeaderFamily::XForwarded,
                };
                ExternalOriginPolicyConfig::new(
                    policy.trusted_forwarded_host,
                    policy.trusted_forwarded_proto,
                    policy.require_https_external_scheme,
                    policy.strict_forwarded_header_consistency,
                    policy.strip_raw_proxy_headers,
                )
                .with_header_family(family)
            }
            None => ExternalOriginPolicyConfig::secure_defaults(),
        };
        let security = HttpSecurityConfig::new(
            SecurityHeadersConfig::secure_defaults(),
            host_policy,
            trusted_proxy_headers,
            external_origin_policy,
            OperationalRouteAccess::local_only(),
        );
        let request_timeout = RequestTimeout::new(Duration::from_secs(raw.request_timeout_seconds))
            .map_err(|_| {
                ExampleServerError::new(ExampleServerErrorReason::RequestTimeoutInvalid)
            })?;
        let request_body_limit =
            RequestBodyLimitBytes::new(raw.request_body_limit_bytes).map_err(|_| {
                ExampleServerError::new(ExampleServerErrorReason::RequestBodyLimitInvalid)
            })?;
        let metrics_idle_timeout =
            MetricsIdleTimeout::new(Duration::from_secs(raw.metrics_idle_timeout_seconds))
                .map_err(|_| {
                    ExampleServerError::new(ExampleServerErrorReason::MetricsIdleTimeoutInvalid)
                })?;
        let environment = environment_for_profile(raw.application_profile);
        let log_format = match environment {
            ServiceEnvironment::Local | ServiceEnvironment::Dev => LogFormat::PlainText,
            ServiceEnvironment::Staging | ServiceEnvironment::Prod => LogFormat::Json,
        };
        let observability = ObservabilityConfig::new(
            environment,
            log_format,
            false,
            "info".to_owned(),
            metrics_idle_timeout,
        )
        .map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ObservabilityConfigInvalid)
        })?;
        let shutdown_timeout = ShutdownTimeout::new(Duration::from_secs(
            raw.shutdown_timeout_seconds,
        ))
        .map_err(|_| ExampleServerError::new(ExampleServerErrorReason::ShutdownTimeoutInvalid))?;

        Ok(Self {
            application_profile: raw.application_profile,
            http: HttpServerConfig::new(
                bind_address,
                CorsConfig::no_cors(),
                TimeoutConfig::new(request_timeout),
                BodyLimitConfig::new(request_body_limit),
            )
            .with_security_config(security),
            observability,
            shutdown_timeout,
        })
    }

    pub(crate) const fn application_profile(&self) -> AppConfigProfile {
        self.application_profile
    }

    pub(crate) fn http(&self) -> &HttpServerConfig {
        &self.http
    }

    pub(crate) fn observability(&self) -> &ObservabilityConfig {
        &self.observability
    }

    pub(crate) const fn shutdown_timeout(&self) -> ShutdownTimeout {
        self.shutdown_timeout
    }
}

const fn environment_for_profile(profile: AppConfigProfile) -> ServiceEnvironment {
    match profile {
        AppConfigProfile::Local => ServiceEnvironment::Local,
        AppConfigProfile::Staging => ServiceEnvironment::Staging,
        AppConfigProfile::Prod => ServiceEnvironment::Prod,
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
