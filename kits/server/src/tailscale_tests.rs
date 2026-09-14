// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
    let error = TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
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
