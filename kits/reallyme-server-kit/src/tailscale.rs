// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tailscale Service endpoint resolution.
//!
//! Tailscale Services already provide the dynamic "stable name to available
//! Service hosts" layer through MagicDNS and Tailscale's own traffic steering.
//! Server-kit therefore resolves app-kit locators into stable service URLs and
//! lets the local Tailscale resolver handle host selection.

use reallyme_app_kit::{
    AppServiceEndpointResolutionError, AppServiceEndpointResolutionErrorReason,
    AppServiceEndpointResolver, AppServiceEndpointScheme, AppServiceEndpointUrl,
    AppServiceLocatedEndpoints, AppServiceLocator, TailscaleServiceLocator,
};
use serde::Deserialize;
use thiserror::Error;

#[cfg(all(feature = "http", feature = "metrics"))]
use crate::observability::record_service_discovery_resolve;

const MAX_DNS_SUFFIX_BYTES: usize = 253;
const MAX_DNS_LABEL_BYTES: usize = 63;
const DEFAULT_SCHEME: AppServiceEndpointScheme = AppServiceEndpointScheme::Http;
const DEFAULT_PORT: u16 = 80;

/// Raw server-owned Tailscale Service resolver configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TailscaleServiceResolverConfigDocument {
    /// Optional MagicDNS suffix, for example `<tailnet>.ts.net`.
    #[serde(default)]
    pub dns_suffix: Option<String>,
    /// Default endpoint scheme used when a locator omits `scheme`.
    #[serde(default)]
    pub default_scheme: Option<String>,
    /// Default endpoint port used when a locator omits `port`.
    #[serde(default)]
    pub default_port: Option<u16>,
}

/// Resolver that maps a Tailscale Service locator to a stable MagicDNS URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscaleServiceResolver {
    config: TailscaleServiceResolverConfig,
}

impl TailscaleServiceResolver {
    /// Builds a resolver from server JSONC.
    pub fn from_document(
        document: TailscaleServiceResolverConfigDocument,
    ) -> Result<Self, TailscaleResolverConfigError> {
        Ok(Self {
            config: TailscaleServiceResolverConfig::from_document(document)?,
        })
    }

    /// Builds a resolver with local Tailscale resolver defaults.
    pub fn local() -> Self {
        Self {
            config: TailscaleServiceResolverConfig::default(),
        }
    }

    fn resolve_tailscale_service(
        &self,
        locator: &TailscaleServiceLocator,
    ) -> Result<AppServiceLocatedEndpoints, AppServiceEndpointResolutionError> {
        let scheme = locator.scheme().unwrap_or(self.config.default_scheme);
        let port = locator.port().unwrap_or(self.config.default_port);
        if port == 0 {
            record_resolve_metric("tailscale_service", "failure");
            return Err(AppServiceEndpointResolutionError::new(
                AppServiceEndpointResolutionErrorReason::InvalidEndpointUrl,
            ));
        }

        let endpoint = build_tailscale_service_url(
            scheme,
            locator.service(),
            self.config.dns_suffix.as_deref(),
            port,
        )
        .map_err(|_| {
            record_resolve_metric("tailscale_service", "failure");
            AppServiceEndpointResolutionError::new(
                AppServiceEndpointResolutionErrorReason::InvalidEndpointUrl,
            )
        })
        .and_then(|endpoint| {
            AppServiceEndpointUrl::new(endpoint).map_err(|_| {
                record_resolve_metric("tailscale_service", "failure");
                AppServiceEndpointResolutionError::new(
                    AppServiceEndpointResolutionErrorReason::InvalidEndpointUrl,
                )
            })
        })?;

        record_resolve_metric("tailscale_service", "success");
        AppServiceLocatedEndpoints::new(vec![endpoint])
    }
}

impl Default for TailscaleServiceResolver {
    fn default() -> Self {
        Self::local()
    }
}

impl AppServiceEndpointResolver for TailscaleServiceResolver {
    fn resolve(
        &self,
        locator: &AppServiceLocator,
    ) -> Result<AppServiceLocatedEndpoints, AppServiceEndpointResolutionError> {
        match locator {
            AppServiceLocator::Tailscale(tailscale) => self.resolve_tailscale_service(tailscale),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TailscaleServiceResolverConfig {
    dns_suffix: Option<String>,
    default_scheme: AppServiceEndpointScheme,
    default_port: u16,
}

impl TailscaleServiceResolverConfig {
    fn from_document(
        document: TailscaleServiceResolverConfigDocument,
    ) -> Result<Self, TailscaleResolverConfigError> {
        let dns_suffix = document.dns_suffix.map(validate_dns_suffix).transpose()?;
        let default_scheme = document
            .default_scheme
            .as_deref()
            .map(parse_scheme)
            .transpose()?
            .unwrap_or(DEFAULT_SCHEME);
        let default_port = document.default_port.unwrap_or(DEFAULT_PORT);
        if default_port == 0 {
            return Err(TailscaleResolverConfigError::new(
                TailscaleResolverConfigErrorReason::InvalidPort,
            ));
        }

        Ok(Self {
            dns_suffix,
            default_scheme,
            default_port,
        })
    }
}

impl Default for TailscaleServiceResolverConfig {
    fn default() -> Self {
        Self {
            dns_suffix: None,
            default_scheme: DEFAULT_SCHEME,
            default_port: DEFAULT_PORT,
        }
    }
}

fn build_tailscale_service_url(
    scheme: AppServiceEndpointScheme,
    service: &str,
    dns_suffix: Option<&str>,
    port: u16,
) -> Result<String, TailscaleResolverConfigError> {
    validate_dns_label(service)?;
    let host = match dns_suffix {
        Some(suffix) => {
            validate_dns_suffix(suffix.to_owned())?;
            let capacity = service
                .len()
                .checked_add(1)
                .and_then(|value| value.checked_add(suffix.len()))
                .ok_or_else(|| {
                    TailscaleResolverConfigError::new(
                        TailscaleResolverConfigErrorReason::InvalidDnsSuffix,
                    )
                })?;
            let mut host = String::with_capacity(capacity);
            host.push_str(service);
            host.push('.');
            host.push_str(suffix);
            host
        }
        None => service.to_owned(),
    };

    let mut endpoint = String::new();
    endpoint.push_str(scheme.as_str());
    endpoint.push_str("://");
    endpoint.push_str(host.as_str());
    endpoint.push(':');
    endpoint.push_str(port.to_string().as_str());
    Ok(endpoint)
}

fn parse_scheme(value: &str) -> Result<AppServiceEndpointScheme, TailscaleResolverConfigError> {
    match value {
        "http" => Ok(AppServiceEndpointScheme::Http),
        "https" => Ok(AppServiceEndpointScheme::Https),
        _ => Err(TailscaleResolverConfigError::new(
            TailscaleResolverConfigErrorReason::InvalidScheme,
        )),
    }
}

fn validate_dns_suffix(value: String) -> Result<String, TailscaleResolverConfigError> {
    if value.is_empty() || value.len() > MAX_DNS_SUFFIX_BYTES {
        return Err(TailscaleResolverConfigError::new(
            TailscaleResolverConfigErrorReason::InvalidDnsSuffix,
        ));
    }
    for label in value.split('.') {
        validate_dns_label(label)?;
    }
    Ok(value)
}

fn validate_dns_label(value: &str) -> Result<(), TailscaleResolverConfigError> {
    if value.is_empty() || value.len() > MAX_DNS_LABEL_BYTES {
        return Err(TailscaleResolverConfigError::new(
            TailscaleResolverConfigErrorReason::InvalidDnsSuffix,
        ));
    }
    let bytes = value.as_bytes();
    let Some(first) = bytes.first() else {
        return Err(TailscaleResolverConfigError::new(
            TailscaleResolverConfigErrorReason::InvalidDnsSuffix,
        ));
    };
    let Some(last) = bytes.last() else {
        return Err(TailscaleResolverConfigError::new(
            TailscaleResolverConfigErrorReason::InvalidDnsSuffix,
        ));
    };
    if *first == b'-' || *last == b'-' {
        return Err(TailscaleResolverConfigError::new(
            TailscaleResolverConfigErrorReason::InvalidDnsSuffix,
        ));
    }
    if bytes
        .iter()
        .any(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-'))
    {
        return Err(TailscaleResolverConfigError::new(
            TailscaleResolverConfigErrorReason::InvalidDnsSuffix,
        ));
    }
    Ok(())
}

#[cfg(all(feature = "http", feature = "metrics"))]
fn record_resolve_metric(source: &'static str, outcome: &'static str) {
    record_service_discovery_resolve(source, outcome);
}

#[cfg(not(all(feature = "http", feature = "metrics")))]
fn record_resolve_metric(_source: &'static str, _outcome: &'static str) {}

/// Typed Tailscale resolver configuration error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("Tailscale service resolver configuration failed")]
pub struct TailscaleResolverConfigError {
    reason: TailscaleResolverConfigErrorReason,
}

impl TailscaleResolverConfigError {
    const fn new(reason: TailscaleResolverConfigErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the low-cardinality resolver configuration failure reason.
    pub const fn reason(self) -> TailscaleResolverConfigErrorReason {
        self.reason
    }
}

/// Low-cardinality Tailscale resolver configuration failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailscaleResolverConfigErrorReason {
    /// MagicDNS suffix was malformed.
    InvalidDnsSuffix,
    /// Default or locator scheme was unsupported.
    InvalidScheme,
    /// Default or locator port was invalid.
    InvalidPort,
}

#[cfg(test)]
mod tests {
    use reallyme_app_kit::{
        AppServiceEndpointResolutionErrorReason, AppServiceEndpointResolver, AppServiceLocator,
        AppServiceLocatorDocument,
    };

    use super::{
        TailscaleResolverConfigErrorReason, TailscaleServiceResolver,
        TailscaleServiceResolverConfigDocument,
    };

    fn locator(service: &str, port: Option<u16>, scheme: Option<&str>) -> AppServiceLocator {
        AppServiceLocator::from_document(AppServiceLocatorDocument {
            mode: "tailscale_service".to_owned(),
            service: service.to_owned(),
            tags: Vec::new(),
            port,
            scheme: scheme.map(str::to_owned),
        })
        .expect("test locator should parse")
    }

    #[test]
    fn resolver_builds_magic_dns_service_url() {
        let resolver =
            TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
                dns_suffix: Some("example.ts.net".to_owned()),
                default_scheme: Some("http".to_owned()),
                default_port: Some(8108),
            })
            .expect("resolver config should validate");

        let endpoints = resolver
            .resolve(&locator("search", None, None))
            .expect("tailscale service should resolve to MagicDNS URL");

        assert_eq!(
            endpoints.endpoints()[0].as_str(),
            "http://search.example.ts.net:8108"
        );
    }

    #[test]
    fn locator_overrides_default_scheme_and_port() {
        let resolver =
            TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
                dns_suffix: Some("example.ts.net".to_owned()),
                default_scheme: Some("http".to_owned()),
                default_port: Some(8108),
            })
            .expect("resolver config should validate");

        let endpoints = resolver
            .resolve(&locator("search", Some(443), Some("https")))
            .expect("tailscale service should resolve to MagicDNS URL");

        assert_eq!(
            endpoints.endpoints()[0].as_str(),
            "https://search.example.ts.net:443"
        );
    }

    #[test]
    fn resolver_can_delegate_short_name_to_local_tailscale_dns_search() {
        let resolver = TailscaleServiceResolver::local();

        let endpoints = resolver
            .resolve(&locator("search", Some(8108), Some("http")))
            .expect("short service name should remain resolvable by local DNS");

        assert_eq!(endpoints.endpoints()[0].as_str(), "http://search:8108");
    }

    #[test]
    fn local_resolver_defaults_to_plain_http_port() {
        let resolver = TailscaleServiceResolver::local();

        let endpoints = resolver
            .resolve(&locator("search", None, None))
            .expect("short service name should resolve with safe URL defaults");

        assert_eq!(endpoints.endpoints()[0].as_str(), "http://search:80");
    }

    #[test]
    fn rejects_invalid_dns_suffix() {
        let error =
            TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
                dns_suffix: Some("-bad.ts.net".to_owned()),
                default_scheme: None,
                default_port: None,
            })
            .expect_err("invalid suffix should fail closed");

        assert_eq!(
            error.reason(),
            TailscaleResolverConfigErrorReason::InvalidDnsSuffix
        );
    }

    #[test]
    fn invalid_locator_service_name_does_not_resolve() {
        let resolver = TailscaleServiceResolver::local();
        let error = resolver
            .resolve(&locator("bad_service", Some(8108), Some("http")))
            .expect_err("invalid DNS label should fail closed");

        assert_eq!(
            error.reason(),
            AppServiceEndpointResolutionErrorReason::InvalidEndpointUrl
        );
    }
}
