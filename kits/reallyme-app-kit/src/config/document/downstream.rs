// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;

use super::error::{AppConfigDocumentError, AppConfigDocumentErrorReason};
use super::raw::RawDownstreamEndpointConfig;
pub use super::service_endpoint::{
    AppServiceEndpointResolutionError, AppServiceEndpointResolver, AppServiceEndpointSelection,
    AppServiceEndpointSource, AppServiceEndpointUrl, AppServiceLocatedEndpoints, AppServiceLocator,
    AppServiceStaticEndpoints,
};
use super::url::AppDownstreamBaseUrl;
use crate::ports::AppPortName;

/// Standard downstream endpoint collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDownstreamConfig {
    endpoints: BTreeMap<AppPortName, AppDownstreamEndpointConfig>,
}

impl AppDownstreamConfig {
    /// Returns a downstream endpoint by typed port name.
    pub fn endpoint(&self, name: &AppPortName) -> Option<&AppDownstreamEndpointConfig> {
        self.endpoints.get(name)
    }

    /// Returns the typed endpoint map.
    pub fn endpoints(&self) -> &BTreeMap<AppPortName, AppDownstreamEndpointConfig> {
        &self.endpoints
    }

    /// Resolves locator-backed downstreams into the same static endpoint shape.
    ///
    /// This is a host-side normalization step. App code can continue reading
    /// `base_url` or `endpoints` without knowing whether the values came from
    /// JSONC, Tailscale Services metadata, MagicDNS, or Tailscale announcements.
    pub fn resolve_with_endpoint_resolver(
        &self,
        endpoint_resolver: &dyn AppServiceEndpointResolver,
    ) -> Result<Self, AppServiceEndpointResolutionError> {
        let mut endpoints = BTreeMap::new();
        for (name, endpoint) in &self.endpoints {
            endpoints.insert(
                name.clone(),
                endpoint.resolve_with_endpoint_resolver(endpoint_resolver)?,
            );
        }

        Ok(Self { endpoints })
    }
}

impl TryFrom<BTreeMap<String, RawDownstreamEndpointConfig>> for AppDownstreamConfig {
    type Error = AppConfigDocumentError;

    fn try_from(raw: BTreeMap<String, RawDownstreamEndpointConfig>) -> Result<Self, Self::Error> {
        let mut endpoints = BTreeMap::new();

        for (name, endpoint) in raw {
            let port_name = AppPortName::new(name).map_err(|_| {
                AppConfigDocumentError::new(AppConfigDocumentErrorReason::InvalidDownstreamName)
            })?;
            endpoints.insert(port_name, AppDownstreamEndpointConfig::try_from(endpoint)?);
        }

        Ok(Self { endpoints })
    }
}

/// Standard downstream endpoint config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDownstreamEndpointConfig {
    source: AppServiceEndpointSource,
}

impl AppDownstreamEndpointConfig {
    /// Returns the endpoint source.
    pub const fn source(&self) -> &AppServiceEndpointSource {
        &self.source
    }

    /// Returns downstream base URL when this is a singleton static endpoint.
    pub fn base_url(&self) -> Option<AppDownstreamBaseUrl> {
        self.source
            .primary_static_endpoint()
            .and_then(|endpoint| AppDownstreamBaseUrl::new(endpoint.as_str()).ok())
    }

    /// Returns all static endpoint URLs, when statically configured.
    pub fn endpoints(&self) -> Option<&[AppServiceEndpointUrl]> {
        self.source
            .static_endpoints()
            .map(AppServiceStaticEndpoints::endpoints)
    }

    /// Returns endpoint selection policy.
    pub const fn endpoint_selection(&self) -> AppServiceEndpointSelection {
        self.source.endpoint_selection()
    }

    /// Returns service-locator metadata, if configured.
    pub const fn locator(&self) -> Option<&AppServiceLocator> {
        self.source.locator()
    }

    /// Resolves this endpoint when it is locator-backed.
    pub fn resolve_with_endpoint_resolver(
        &self,
        endpoint_resolver: &dyn AppServiceEndpointResolver,
    ) -> Result<Self, AppServiceEndpointResolutionError> {
        let static_endpoints = self.source.static_endpoints();
        let Some(locator) = self.source.locator() else {
            return Ok(self.clone());
        };
        let located_endpoints = match endpoint_resolver.resolve(locator) {
            Ok(located_endpoints) => located_endpoints,
            Err(error) => {
                let Some(static_endpoints) = static_endpoints else {
                    return Err(error);
                };

                return Ok(Self {
                    source: AppServiceEndpointSource::from_located_endpoints(
                        AppServiceLocatedEndpoints::new(static_endpoints.endpoints().to_vec())?,
                        self.source.endpoint_selection(),
                    )?,
                });
            }
        };
        let mut endpoints = located_endpoints.endpoints().to_vec();
        if let Some(static_endpoints) = static_endpoints {
            append_unique_endpoints(&mut endpoints, static_endpoints.endpoints());
        }

        Ok(Self {
            source: AppServiceEndpointSource::from_located_endpoints(
                AppServiceLocatedEndpoints::new(endpoints)?,
                self.source.endpoint_selection(),
            )?,
        })
    }
}

impl TryFrom<RawDownstreamEndpointConfig> for AppDownstreamEndpointConfig {
    type Error = AppConfigDocumentError;

    fn try_from(raw: RawDownstreamEndpointConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            source: AppServiceEndpointSource::from_document(raw)?,
        })
    }
}

fn append_unique_endpoints(
    endpoints: &mut Vec<AppServiceEndpointUrl>,
    candidates: &[AppServiceEndpointUrl],
) {
    for candidate in candidates {
        if endpoints
            .iter()
            .any(|endpoint| endpoint.as_str() == candidate.as_str())
        {
            continue;
        }

        endpoints.push(candidate.clone());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        AppDownstreamConfig, AppDownstreamEndpointConfig, AppServiceEndpointResolutionError,
        AppServiceEndpointResolver, AppServiceEndpointUrl, AppServiceLocator,
        RawDownstreamEndpointConfig,
    };
    use crate::config::document::{
        AppServiceEndpointResolutionErrorReason, AppServiceLocatedEndpoints,
        AppServiceLocatorDocument,
    };

    #[test]
    fn downstream_config_resolves_locator_backed_endpoint() {
        let config = AppDownstreamConfig::try_from(BTreeMap::from([(
            "search_index".to_owned(),
            RawDownstreamEndpointConfig {
                base_url: None,
                endpoints: Vec::new(),
                endpoint_selection: Some("round_robin".to_owned()),
                locator: Some(AppServiceLocatorDocument {
                    mode: "tailscale".to_owned(),
                    service: "search".to_owned(),
                    tags: vec!["prod".to_owned(), "eu".to_owned()],
                    port: Some(8108),
                    scheme: Some("http".to_owned()),
                }),
            },
        )]))
        .expect("downstream fixture should validate");

        let resolved = config
            .resolve_with_endpoint_resolver(&StaticResolver)
            .expect("locator should resolve");
        let endpoint = resolved
            .endpoint(&crate::ports::AppPortName::new("search_index").expect("valid port"))
            .expect("resolved endpoint should exist");

        assert_eq!(endpoint.locator(), None);
        assert_eq!(
            endpoint
                .endpoints()
                .and_then(|endpoints| endpoints.first())
                .map(AppServiceEndpointUrl::as_str),
            Some("http://100.64.0.10:8108")
        );
    }

    #[test]
    fn downstream_endpoint_static_config_is_preserved() {
        let endpoint = AppDownstreamEndpointConfig::try_from(RawDownstreamEndpointConfig {
            base_url: Some("http://127.0.0.1:8108".to_owned()),
            endpoints: Vec::new(),
            endpoint_selection: None,
            locator: None,
        })
        .expect("static endpoint should validate");

        let resolved = endpoint
            .resolve_with_endpoint_resolver(&StaticResolver)
            .expect("static endpoint should not need resolver");

        assert_eq!(
            resolved
                .base_url()
                .map(|base_url| base_url.as_str().to_owned()),
            Some("http://127.0.0.1:8108".to_owned())
        );
    }

    #[test]
    fn downstream_endpoint_prefers_located_endpoints_and_keeps_static_fallbacks() {
        let endpoint = AppDownstreamEndpointConfig::try_from(RawDownstreamEndpointConfig {
            base_url: Some("http://typesense-fallback:8108".to_owned()),
            endpoints: Vec::new(),
            endpoint_selection: Some("round_robin".to_owned()),
            locator: Some(AppServiceLocatorDocument {
                mode: "tailscale".to_owned(),
                service: "search".to_owned(),
                tags: vec!["prod".to_owned(), "eu".to_owned()],
                port: Some(8108),
                scheme: Some("http".to_owned()),
            }),
        })
        .expect("locator with static fallback should validate");

        let resolved = endpoint
            .resolve_with_endpoint_resolver(&StaticResolver)
            .expect("locator should resolve");
        let endpoint_values = resolved
            .endpoints()
            .expect("resolved endpoint set should be static")
            .iter()
            .map(AppServiceEndpointUrl::as_str)
            .collect::<Vec<_>>();

        assert_eq!(
            endpoint_values,
            ["http://100.64.0.10:8108", "http://typesense-fallback:8108"]
        );
    }

    #[test]
    fn downstream_endpoint_uses_static_fallback_when_locator_resolution_fails() {
        let endpoint = AppDownstreamEndpointConfig::try_from(RawDownstreamEndpointConfig {
            base_url: Some("http://typesense-fallback:8108".to_owned()),
            endpoints: Vec::new(),
            endpoint_selection: Some("failover".to_owned()),
            locator: Some(AppServiceLocatorDocument {
                mode: "tailscale".to_owned(),
                service: "search".to_owned(),
                tags: vec!["prod".to_owned(), "eu".to_owned()],
                port: Some(8108),
                scheme: Some("http".to_owned()),
            }),
        })
        .expect("locator with static fallback should validate");

        let resolved = endpoint
            .resolve_with_endpoint_resolver(&FailingResolver)
            .expect("static fallback should cover locator failure");
        let endpoint_values = resolved
            .endpoints()
            .expect("fallback endpoint set should be static")
            .iter()
            .map(AppServiceEndpointUrl::as_str)
            .collect::<Vec<_>>();

        assert_eq!(endpoint_values, ["http://typesense-fallback:8108"]);
    }

    struct StaticResolver;

    impl AppServiceEndpointResolver for StaticResolver {
        fn resolve(
            &self,
            _locator: &AppServiceLocator,
        ) -> Result<AppServiceLocatedEndpoints, AppServiceEndpointResolutionError> {
            AppServiceLocatedEndpoints::new(vec![
                AppServiceEndpointUrl::new("http://100.64.0.10:8108")
                    .expect("test endpoint should validate"),
            ])
        }
    }

    struct FailingResolver;

    impl AppServiceEndpointResolver for FailingResolver {
        fn resolve(
            &self,
            _locator: &AppServiceLocator,
        ) -> Result<AppServiceLocatedEndpoints, AppServiceEndpointResolutionError> {
            Err(AppServiceEndpointResolutionError::new(
                AppServiceEndpointResolutionErrorReason::ResolverUnavailable,
            ))
        }
    }
}
