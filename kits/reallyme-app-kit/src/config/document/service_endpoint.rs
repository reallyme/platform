// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

use super::error::{AppConfigDocumentError, AppConfigDocumentErrorReason};
use super::url::{UrlPolicy, validate_url};

const MAX_ENDPOINTS: usize = 16;
const MAX_LOCATOR_TAGS: usize = 16;
const MAX_IDENTIFIER_BYTES: usize = 64;

/// Raw reusable endpoint-source document.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppServiceEndpointSourceDocument {
    /// Singleton static endpoint URL.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Static endpoint URLs in preference order.
    #[serde(default)]
    pub endpoints: Vec<String>,
    /// Endpoint selection policy token.
    #[serde(default)]
    pub endpoint_selection: Option<String>,
    /// Optional service locator.
    #[serde(default)]
    pub locator: Option<AppServiceLocatorDocument>,
}

/// Raw reusable service-locator document.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppServiceLocatorDocument {
    /// Locator mode token.
    pub mode: String,
    /// Located Tailscale Service token.
    pub service: String,
    /// Optional Tailscale Service tags retained for policy/debug metadata.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional endpoint port. Host-level resolver defaults may supply this.
    #[serde(default)]
    pub port: Option<u16>,
    /// Optional endpoint URL scheme. Host-level resolver defaults may supply this.
    #[serde(default)]
    pub scheme: Option<String>,
}

/// Validated source for a downstream service endpoint set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServiceEndpointSource {
    source: AppServiceEndpointSourceKind,
    endpoint_selection: AppServiceEndpointSelection,
}

impl AppServiceEndpointSource {
    /// Builds a validated endpoint source from a raw config document.
    pub fn from_document(
        document: AppServiceEndpointSourceDocument,
    ) -> Result<Self, AppConfigDocumentError> {
        let endpoint_selection = document
            .endpoint_selection
            .as_deref()
            .map(AppServiceEndpointSelection::parse)
            .transpose()?
            .unwrap_or(AppServiceEndpointSelection::NearestNode);

        let has_base_url = document.base_url.is_some();
        let has_endpoints = !document.endpoints.is_empty();
        if has_base_url && has_endpoints {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidServiceEndpointSource,
            ));
        }

        let static_endpoints = if let Some(base_url) = document.base_url {
            Some(AppServiceStaticEndpoints::new(vec![
                AppServiceEndpointUrl::new(base_url)?,
            ])?)
        } else if has_endpoints {
            Some(AppServiceStaticEndpoints::new(
                document
                    .endpoints
                    .into_iter()
                    .map(AppServiceEndpointUrl::new)
                    .collect::<Result<Vec<_>, _>>()?,
            )?)
        } else {
            None
        };
        let locator = document
            .locator
            .map(AppServiceLocator::from_document)
            .transpose()?;

        let source = match (static_endpoints, locator) {
            (Some(endpoints), Some(locator)) => AppServiceEndpointSourceKind::StaticWithLocator(
                AppServiceStaticEndpointsWithLocator { endpoints, locator },
            ),
            (Some(endpoints), None) => AppServiceEndpointSourceKind::Static(endpoints),
            (None, Some(locator)) => AppServiceEndpointSourceKind::Locator(locator),
            (None, None) => {
                return Err(AppConfigDocumentError::new(
                    AppConfigDocumentErrorReason::InvalidServiceEndpointSource,
                ));
            }
        };

        Ok(Self {
            source,
            endpoint_selection,
        })
    }

    /// Returns the endpoint selection policy.
    pub const fn endpoint_selection(&self) -> AppServiceEndpointSelection {
        self.endpoint_selection
    }

    /// Returns static endpoints if this source is statically configured.
    pub const fn static_endpoints(&self) -> Option<&AppServiceStaticEndpoints> {
        match &self.source {
            AppServiceEndpointSourceKind::Static(endpoints) => Some(endpoints),
            AppServiceEndpointSourceKind::StaticWithLocator(source) => Some(&source.endpoints),
            AppServiceEndpointSourceKind::Locator(_) => None,
        }
    }

    /// Returns locator metadata if this source is locator-backed.
    pub const fn locator(&self) -> Option<&AppServiceLocator> {
        match &self.source {
            AppServiceEndpointSourceKind::Static(_) => None,
            AppServiceEndpointSourceKind::StaticWithLocator(source) => Some(&source.locator),
            AppServiceEndpointSourceKind::Locator(locator) => Some(locator),
        }
    }

    /// Returns the primary static endpoint, if available.
    pub fn primary_static_endpoint(&self) -> Option<&AppServiceEndpointUrl> {
        self.static_endpoints()
            .and_then(|endpoints| endpoints.endpoints().first())
    }

    /// Builds a static source from endpoints returned by a runtime resolver.
    pub fn from_located_endpoints(
        located_endpoints: AppServiceLocatedEndpoints,
        endpoint_selection: AppServiceEndpointSelection,
    ) -> Result<Self, AppServiceEndpointResolutionError> {
        let static_endpoints = AppServiceStaticEndpoints::new(
            located_endpoints.endpoints().to_vec(),
        )
        .map_err(|_| {
            AppServiceEndpointResolutionError::new(
                AppServiceEndpointResolutionErrorReason::InvalidEndpointSet,
            )
        })?;

        Ok(Self {
            source: AppServiceEndpointSourceKind::Static(static_endpoints),
            endpoint_selection,
        })
    }
}

/// Concrete endpoint source kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppServiceEndpointSourceKind {
    /// Static endpoint list from config.
    Static(AppServiceStaticEndpoints),
    /// Locator-backed endpoint source.
    Locator(AppServiceLocator),
    /// Locator-backed source with static fallback endpoints.
    StaticWithLocator(AppServiceStaticEndpointsWithLocator),
}

/// Static fallback endpoints paired with a service locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServiceStaticEndpointsWithLocator {
    endpoints: AppServiceStaticEndpoints,
    locator: AppServiceLocator,
}

/// Static service endpoint list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServiceStaticEndpoints {
    endpoints: Vec<AppServiceEndpointUrl>,
}

impl AppServiceStaticEndpoints {
    /// Constructs a validated static endpoint set.
    pub fn new(endpoints: Vec<AppServiceEndpointUrl>) -> Result<Self, AppConfigDocumentError> {
        if endpoints.is_empty() || endpoints.len() > MAX_ENDPOINTS {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidServiceEndpointSource,
            ));
        }

        Ok(Self { endpoints })
    }

    /// Returns validated static endpoints in configured preference order.
    pub fn endpoints(&self) -> &[AppServiceEndpointUrl] {
        self.endpoints.as_slice()
    }
}

/// Runtime service-locator resolver supplied by the host process.
///
/// App crates use this trait to stay independent of the concrete discovery
/// adapter. Native servers can back it with Tailscale Services metadata,
/// MagicDNS, or Tailscale LAN announcements without teaching app config about
/// those transport details.
pub trait AppServiceEndpointResolver: Send + Sync {
    /// Resolves a locator into concrete HTTP(S) endpoint URLs.
    fn resolve(
        &self,
        locator: &AppServiceLocator,
    ) -> Result<AppServiceLocatedEndpoints, AppServiceEndpointResolutionError>;
}

/// Endpoint set returned by a runtime service-locator resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServiceLocatedEndpoints {
    endpoints: Vec<AppServiceEndpointUrl>,
}

impl AppServiceLocatedEndpoints {
    /// Constructs a validated located endpoint set.
    pub fn new(
        endpoints: Vec<AppServiceEndpointUrl>,
    ) -> Result<Self, AppServiceEndpointResolutionError> {
        if endpoints.is_empty() || endpoints.len() > MAX_ENDPOINTS {
            return Err(AppServiceEndpointResolutionError::new(
                AppServiceEndpointResolutionErrorReason::InvalidEndpointSet,
            ));
        }

        Ok(Self { endpoints })
    }

    /// Returns located endpoints in resolver preference order.
    pub fn endpoints(&self) -> &[AppServiceEndpointUrl] {
        self.endpoints.as_slice()
    }
}

/// Typed service-locator resolution error.
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
#[error("service endpoint resolution failed")]
pub struct AppServiceEndpointResolutionError {
    reason: AppServiceEndpointResolutionErrorReason,
}

impl AppServiceEndpointResolutionError {
    /// Constructs a typed service-locator resolution error.
    pub const fn new(reason: AppServiceEndpointResolutionErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the low-cardinality resolution failure reason.
    pub const fn reason(self) -> AppServiceEndpointResolutionErrorReason {
        self.reason
    }
}

/// Low-cardinality service-locator resolution failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppServiceEndpointResolutionErrorReason {
    /// The resolver does not support the requested locator provider.
    UnsupportedLocatorProvider,
    /// The resolver is not ready or has no discovery source available.
    ResolverUnavailable,
    /// The locator matched no currently available endpoints.
    EndpointNotFound,
    /// The discovery source returned an invalid endpoint set.
    InvalidEndpointSet,
    /// The discovery source returned an invalid endpoint URL.
    InvalidEndpointUrl,
}

/// Validated HTTP(S) service endpoint.
#[derive(Clone, PartialEq, Eq)]
pub struct AppServiceEndpointUrl(String);

impl AppServiceEndpointUrl {
    /// Constructs a validated endpoint URL.
    pub fn new(value: impl Into<String>) -> Result<Self, AppConfigDocumentError> {
        let value = value.into();
        validate_url(value.as_str(), UrlPolicy::HttpOrHttps)?;
        Ok(Self(value))
    }

    /// Returns the endpoint URL.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Debug for AppServiceEndpointUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("AppServiceEndpointUrl")
            .field(&"[REDACTED]")
            .finish()
    }
}

/// Endpoint selection policy for static or located endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppServiceEndpointSelection {
    /// Prefer the first/nearest endpoint.
    NearestNode,
    /// Rotate primary endpoint per request.
    RoundRobin,
    /// Use endpoints as ordered failover candidates.
    Failover,
}

impl AppServiceEndpointSelection {
    /// Parses a stable endpoint-selection token.
    pub fn parse(value: &str) -> Result<Self, AppConfigDocumentError> {
        match value {
            "nearest_node" => Ok(Self::NearestNode),
            "round_robin" => Ok(Self::RoundRobin),
            "failover" => Ok(Self::Failover),
            _ => Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidEndpointSelection,
            )),
        }
    }

    /// Returns the stable endpoint-selection token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NearestNode => "nearest_node",
            Self::RoundRobin => "round_robin",
            Self::Failover => "failover",
        }
    }
}

/// Validated service locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppServiceLocator {
    /// Tailscale service locator.
    Tailscale(TailscaleServiceLocator),
}

impl AppServiceLocator {
    /// Builds a validated service locator from a raw config document.
    pub fn from_document(
        document: AppServiceLocatorDocument,
    ) -> Result<Self, AppConfigDocumentError> {
        match document.mode.as_str() {
            "tailscale" | "tailscale_service" => Ok(Self::Tailscale(
                TailscaleServiceLocator::from_document(document)?,
            )),
            _ => Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidLocatorMode,
            )),
        }
    }

    /// Returns the locator provider token.
    pub const fn provider(&self) -> AppServiceLocatorProvider {
        match self {
            Self::Tailscale(_) => AppServiceLocatorProvider::Tailscale,
        }
    }

    /// Returns Tailscale locator metadata when applicable.
    pub const fn tailscale(&self) -> Option<&TailscaleServiceLocator> {
        match self {
            Self::Tailscale(locator) => Some(locator),
        }
    }
}

/// Supported service-locator providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppServiceLocatorProvider {
    /// Tailscale Services MagicDNS.
    Tailscale,
}

impl AppServiceLocatorProvider {
    /// Returns the stable provider token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tailscale => "tailscale_service",
        }
    }
}

/// Tailscale Service locator selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscaleServiceLocator {
    service: String,
    tags: Vec<String>,
    port: Option<u16>,
    scheme: Option<AppServiceEndpointScheme>,
}

impl TailscaleServiceLocator {
    fn from_document(document: AppServiceLocatorDocument) -> Result<Self, AppConfigDocumentError> {
        validate_identifier(document.service.as_str())?;
        if document.tags.len() > MAX_LOCATOR_TAGS {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidLocatorTag,
            ));
        }
        for tag in &document.tags {
            validate_identifier(tag.as_str())?;
        }
        let scheme = document
            .scheme
            .as_deref()
            .map(AppServiceEndpointScheme::parse)
            .transpose()?;
        if document.port == Some(0) {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidServiceEndpointSource,
            ));
        }

        Ok(Self {
            service: document.service,
            tags: document.tags,
            port: document.port,
            scheme,
        })
    }

    /// Returns the Tailscale Service token.
    pub fn service(&self) -> &str {
        self.service.as_str()
    }

    /// Returns configured Tailscale Service metadata tags.
    pub fn tags(&self) -> &[String] {
        self.tags.as_slice()
    }

    /// Returns the configured endpoint port, if present.
    pub const fn port(&self) -> Option<u16> {
        self.port
    }

    /// Returns the configured endpoint scheme, if present.
    pub const fn scheme(&self) -> Option<AppServiceEndpointScheme> {
        self.scheme
    }
}

/// Supported URL schemes for Tailscale Service endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppServiceEndpointScheme {
    /// Plain HTTP over the tailnet.
    Http,
    /// HTTPS over the tailnet.
    Https,
}

impl AppServiceEndpointScheme {
    fn parse(value: &str) -> Result<Self, AppConfigDocumentError> {
        match value {
            "http" => Ok(Self::Http),
            "https" => Ok(Self::Https),
            _ => Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidServiceEndpointSource,
            )),
        }
    }

    /// Returns the URL scheme token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

fn validate_identifier(value: &str) -> Result<(), AppConfigDocumentError> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InvalidLocatorIdentifier,
        ));
    }

    if value.bytes().any(|byte| {
        !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_')
    }) {
        return Err(AppConfigDocumentError::new(
            AppConfigDocumentErrorReason::InvalidLocatorIdentifier,
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AppServiceEndpointSelection, AppServiceEndpointSource, AppServiceEndpointSourceDocument,
        AppServiceLocatorDocument,
    };
    use crate::config::document::AppConfigDocumentErrorReason;

    #[test]
    fn singleton_base_url_source_normalizes_to_static_endpoint() {
        let source = AppServiceEndpointSource::from_document(AppServiceEndpointSourceDocument {
            base_url: Some("http://typesense-1:8108".to_owned()),
            endpoints: Vec::new(),
            endpoint_selection: None,
            locator: None,
        })
        .expect("singleton endpoint source should validate");

        assert_eq!(
            source
                .primary_static_endpoint()
                .map(|endpoint| endpoint.as_str()),
            Some("http://typesense-1:8108")
        );
        assert_eq!(
            source.endpoint_selection(),
            AppServiceEndpointSelection::NearestNode
        );
    }

    #[test]
    fn static_endpoint_list_preserves_selection_policy() {
        let source = AppServiceEndpointSource::from_document(AppServiceEndpointSourceDocument {
            base_url: None,
            endpoints: vec![
                "http://typesense-1:8108".to_owned(),
                "http://typesense-2:8108".to_owned(),
            ],
            endpoint_selection: Some("round_robin".to_owned()),
            locator: None,
        })
        .expect("static endpoint list should validate");

        let endpoints = source
            .static_endpoints()
            .expect("static source should expose endpoints");
        assert_eq!(endpoints.endpoints().len(), 2);
        assert_eq!(
            source.endpoint_selection(),
            AppServiceEndpointSelection::RoundRobin
        );
    }

    #[test]
    fn tailscale_locator_source_validates_tags() {
        let source = AppServiceEndpointSource::from_document(AppServiceEndpointSourceDocument {
            base_url: None,
            endpoints: Vec::new(),
            endpoint_selection: Some("failover".to_owned()),
            locator: Some(AppServiceLocatorDocument {
                mode: "tailscale".to_owned(),
                service: "typesense".to_owned(),
                tags: vec!["prod".to_owned(), "search".to_owned(), "eu".to_owned()],
                port: Some(8108),
                scheme: Some("http".to_owned()),
            }),
        })
        .expect("tailscale locator source should validate");

        let locator = source
            .locator()
            .and_then(super::AppServiceLocator::tailscale)
            .expect("tailscale locator should be retained");
        assert_eq!(locator.service(), "typesense");
        assert_eq!(locator.tags(), ["prod", "search", "eu"]);
        assert_eq!(
            source.endpoint_selection(),
            AppServiceEndpointSelection::Failover
        );
    }

    #[test]
    fn endpoint_source_accepts_static_fallback_with_tailscale_locator() {
        let source = AppServiceEndpointSource::from_document(AppServiceEndpointSourceDocument {
            base_url: Some("http://typesense-1:8108".to_owned()),
            endpoints: Vec::new(),
            endpoint_selection: Some("round_robin".to_owned()),
            locator: Some(AppServiceLocatorDocument {
                mode: "tailscale".to_owned(),
                service: "typesense".to_owned(),
                tags: vec!["prod".to_owned(), "search".to_owned()],
                port: Some(8108),
                scheme: Some("http".to_owned()),
            }),
        })
        .expect("static fallback with tailscale locator should validate");

        assert_eq!(
            source
                .primary_static_endpoint()
                .map(super::AppServiceEndpointUrl::as_str),
            Some("http://typesense-1:8108")
        );
        assert_eq!(
            source
                .locator()
                .and_then(super::AppServiceLocator::tailscale)
                .map(|locator| locator.service()),
            Some("typesense")
        );
        assert_eq!(
            source.endpoint_selection(),
            AppServiceEndpointSelection::RoundRobin
        );
    }

    #[test]
    fn endpoint_source_rejects_base_url_and_endpoint_list_together() {
        let error = AppServiceEndpointSource::from_document(AppServiceEndpointSourceDocument {
            base_url: Some("http://typesense-1:8108".to_owned()),
            endpoints: vec!["http://typesense-2:8108".to_owned()],
            endpoint_selection: None,
            locator: Some(AppServiceLocatorDocument {
                mode: "tailscale".to_owned(),
                service: "typesense".to_owned(),
                tags: vec!["prod".to_owned(), "search".to_owned()],
                port: Some(8108),
                scheme: Some("http".to_owned()),
            }),
        })
        .expect_err("base_url and endpoints should fail closed");

        assert_eq!(
            error.reason(),
            AppConfigDocumentErrorReason::InvalidServiceEndpointSource
        );
    }

    #[test]
    fn tailscale_service_locator_allows_empty_tags() {
        let source = AppServiceEndpointSource::from_document(AppServiceEndpointSourceDocument {
            base_url: None,
            endpoints: Vec::new(),
            endpoint_selection: None,
            locator: Some(AppServiceLocatorDocument {
                mode: "tailscale".to_owned(),
                service: "typesense".to_owned(),
                tags: Vec::new(),
                port: Some(8108),
                scheme: Some("http".to_owned()),
            }),
        })
        .expect("Tailscale Service MagicDNS locators do not require tag selectors");

        assert_eq!(
            source
                .locator()
                .and_then(super::AppServiceLocator::tailscale)
                .map(|locator| locator.service()),
            Some("typesense")
        );
    }

    #[test]
    fn located_endpoints_normalize_to_static_source() {
        let located = super::AppServiceLocatedEndpoints::new(vec![
            super::AppServiceEndpointUrl::new("http://typesense-1:8108")
                .expect("test endpoint should validate"),
        ])
        .expect("located endpoint set should validate");
        let source = AppServiceEndpointSource::from_located_endpoints(
            located,
            AppServiceEndpointSelection::RoundRobin,
        )
        .expect("located endpoints should normalize");

        assert!(source.locator().is_none());
        assert_eq!(
            source
                .primary_static_endpoint()
                .map(super::AppServiceEndpointUrl::as_str),
            Some("http://typesense-1:8108")
        );
        assert_eq!(
            source.endpoint_selection(),
            AppServiceEndpointSelection::RoundRobin
        );
    }
}
