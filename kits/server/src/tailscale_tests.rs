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
            default_scheme: Some("https".to_owned()),
            default_port: Some(8108),
        })
        .expect("resolver config should validate");

    let endpoints = resolver
        .resolve(&locator("search", None, None))
        .expect("tailscale service should resolve to MagicDNS URL");

    assert_eq!(
        endpoints.endpoints()[0].as_str(),
        "https://search.example.ts.net:8108"
    );
}

#[test]
fn locator_overrides_default_scheme_and_port() {
    let resolver =
        TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
            dns_suffix: Some("example.ts.net".to_owned()),
            default_scheme: Some("https".to_owned()),
            default_port: Some(8108),
        })
        .expect("resolver config should validate");

    let endpoints = resolver
        .resolve(&locator("search", Some(443), Some("https")))
        .expect("tailscale service should resolve to MagicDNS URL");

    assert_eq!(
        endpoints.endpoints()[0].as_str(),
        "https://search.example.ts.net"
    );
}

#[test]
fn resolver_rejects_plaintext_scheme() {
    let error = TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
        dns_suffix: Some("example.ts.net".to_owned()),
        default_scheme: Some("http".to_owned()),
        default_port: Some(8108),
    })
    .expect_err("plaintext locator must fail without transport proof");
    assert_eq!(
        error.reason(),
        TailscaleResolverConfigErrorReason::InvalidScheme
    );
}

#[test]
fn resolver_requires_magic_dns_suffix() {
    let error = TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
        dns_suffix: None,
        default_scheme: None,
        default_port: None,
    })
    .expect_err("a bare service name must not use ambient DNS search paths");
    assert_eq!(
        error.reason(),
        TailscaleResolverConfigErrorReason::MissingDnsSuffix
    );
}

#[test]
fn resolver_defaults_to_https_port() {
    let resolver =
        TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
            dns_suffix: Some("example.ts.net".to_owned()),
            default_scheme: None,
            default_port: None,
        })
        .expect("suffix should validate");

    let endpoints = resolver
        .resolve(&locator("search", None, None))
        .expect("service name should resolve with safe URL defaults");

    assert_eq!(
        endpoints.endpoints()[0].as_str(),
        "https://search.example.ts.net"
    );
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
fn rejects_incomplete_or_ambiguous_dns_suffixes() {
    for suffix in [
        "singlelabel",
        ".example.com",
        "example..com",
        "example.com.",
    ] {
        let error =
            TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
                dns_suffix: Some(suffix.to_owned()),
                default_scheme: Some("https".to_owned()),
                default_port: Some(8108),
            })
            .expect_err("resolver must reject invalid DNS suffixes");
        assert_eq!(
            error.reason(),
            TailscaleResolverConfigErrorReason::InvalidDnsSuffix
        );
    }
}

#[test]
fn accepts_configured_headscale_dns_suffix() {
    let resolver =
        TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
            dns_suffix: Some("nodes.internal.example".to_owned()),
            default_scheme: None,
            default_port: None,
        });
    assert!(resolver.is_ok());
}

#[test]
fn invalid_locator_service_name_does_not_resolve() {
    let resolver =
        TailscaleServiceResolver::from_document(TailscaleServiceResolverConfigDocument {
            dns_suffix: Some("example.ts.net".to_owned()),
            default_scheme: None,
            default_port: None,
        })
        .expect("suffix should validate");
    let error = resolver
        .resolve(&locator("bad_service", Some(8108), Some("https")))
        .expect_err("invalid DNS label should fail closed");

    assert_eq!(
        error.reason(),
        AppServiceEndpointResolutionErrorReason::InvalidEndpointUrl
    );
}
