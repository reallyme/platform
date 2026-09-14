// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;

use super::{
    AppDownstreamConfig, AppDownstreamEndpointConfig, AppServiceEndpointResolutionError,
    AppServiceEndpointResolver, AppServiceEndpointUrl, AppServiceLocator,
    RawDownstreamEndpointConfig,
};
use crate::config::document::{
    AppServiceEndpointResolutionErrorReason, AppServiceLocatedEndpoints, AppServiceLocatorDocument,
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
