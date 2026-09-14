// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
#[path = "downstream_tests.rs"]
mod tests;
