// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
