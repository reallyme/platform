// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use tower::{Layer, Service, ServiceExt};

use crate::authn::{AuthenticatedPrincipal, Principal, PrincipalId, PrincipalKind};
use crate::http::{
    HttpListenerIdentity, HttpListenerName, HttpListenerVisibility, HttpRateLimitTierName,
    HttpRoutePrefix, HttpRouteVisibility, HttpRouteVisibilityPolicy, HttpRouteVisibilityRule,
    RequestBodyLimitBytes,
};

use super::route_visibility_layer;

fn request_with_principal(uri: &'static str, principal_id: &'static str) -> Request<Body> {
    let mut request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("valid request");
    request
        .extensions_mut()
        .insert(Principal::Authenticated(AuthenticatedPrincipal::new(
            PrincipalId::new(principal_id).expect("valid principal id"),
            PrincipalKind::Service,
        )));
    request
}

#[tokio::test]
async fn public_listener_rejects_private_only_route_before_handler_runs() {
    let handler_executed = Arc::new(AtomicBool::new(false));
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![HttpRouteVisibilityRule::new(
        HttpRoutePrefix::new("/internal").expect("valid route prefix"),
        HttpRouteVisibility::PrivateOnly,
    )])
    .expect("unique route rule");
    let app = Router::new()
        .route(
            "/internal/status",
            get(
                |State(handler_executed): State<Arc<AtomicBool>>| async move {
                    handler_executed.store(true, Ordering::SeqCst);
                    StatusCode::OK
                },
            ),
        )
        .with_state(handler_executed.clone());
    let mut service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener name"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/internal/status")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(!handler_executed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn private_listener_allows_private_only_route() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![HttpRouteVisibilityRule::new(
        HttpRoutePrefix::new("/internal").expect("valid route prefix"),
        HttpRouteVisibility::PrivateOnly,
    )])
    .expect("unique route rule");
    let app = Router::new().route("/internal/status", get(|| async { StatusCode::OK }));
    let mut service = route_visibility_layer(
        HttpListenerName::new("private").expect("valid listener name"),
        HttpListenerVisibility::Private,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/internal/status")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn route_visibility_rejects_missing_rule_when_default_is_deny() {
    let policy =
        HttpRouteVisibilityPolicy::deny_by_default(vec![]).expect("policy should be valid");
    let app = Router::new().route("/listed", get(|| async { StatusCode::OK }));
    let mut service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener name"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/unlisted")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn allowed_request_receives_runtime_listener_identity_extension() {
    let policy = HttpRouteVisibilityPolicy::allow_all();
    let app = Router::new().route(
        "/identity",
        get(|request: Request<Body>| async move {
            request
                .extensions()
                .get::<HttpListenerIdentity>()
                .map(|identity| {
                    if identity.name().as_str() == "private-h3"
                        && identity.visibility() == &HttpListenerVisibility::Private
                    {
                        StatusCode::OK
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                })
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
        }),
    );
    let mut service = route_visibility_layer(
        HttpListenerName::new("private-h3").expect("valid listener name"),
        HttpListenerVisibility::Private,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/identity")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn listener_identity_is_runtime_assigned_not_header_inferred() {
    let policy = HttpRouteVisibilityPolicy::allow_all();
    let app = Router::new().route(
        "/identity",
        get(|request: Request<Body>| async move {
            request
                .extensions()
                .get::<HttpListenerIdentity>()
                .map(|identity| {
                    if identity.name().as_str() == "public-ingress"
                        && identity.visibility() == &HttpListenerVisibility::Public
                    {
                        StatusCode::OK
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                })
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
        }),
    );
    let mut service = route_visibility_layer(
        HttpListenerName::new("public-ingress").expect("valid listener name"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/identity")
                .header("host", "private.internal.reallyme.test")
                .header(
                    "forwarded",
                    "for=10.0.0.7;host=private.internal.reallyme.test",
                )
                .header("x-forwarded-host", "private.internal.reallyme.test")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn route_visibility_rejects_request_exceeding_route_body_limit() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![
        HttpRouteVisibilityRule::exact(
            HttpRoutePrefix::new("/handles/handle-availability/check").expect("valid route prefix"),
            HttpRouteVisibility::PublicOnly,
        )
        .with_request_body_limit(Some(
            RequestBodyLimitBytes::new(1024).expect("valid route body limit"),
        )),
    ])
    .expect("unique route rule");
    let app = Router::new().route(
        "/handles/handle-availability/check",
        get(|| async { StatusCode::OK }),
    );
    let mut service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener name"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/handles/handle-availability/check")
                .header("content-length", "2048")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn route_visibility_rate_limiter_rejects_when_bucket_exhausts() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![
        HttpRouteVisibilityRule::exact(
            HttpRoutePrefix::new("/handles/handle-availability/check").expect("valid route prefix"),
            HttpRouteVisibility::PublicOnly,
        )
        .with_rate_limit_tier(Some(
            HttpRateLimitTierName::new("handle-check-public").expect("valid tier"),
        )),
    ])
    .expect("unique route rule");
    let app = Router::new().route(
        "/handles/handle-availability/check",
        get(|| async { StatusCode::OK }),
    );
    let policies = Arc::new(vec![(
        HttpRateLimitTierName::new("handle-check-public").expect("valid tier"),
        crate::runtime::HttpRateLimitTierPolicy::new(0, 1, 100)
            .expect("valid rate-limit tier policy"),
    )]);
    let mut service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener name"),
        HttpListenerVisibility::Public,
        None,
        policies,
        &policy,
    )
    .layer(app);

    let first = service
        .ready()
        .await
        .expect("service should become ready")
        .call(request_with_principal(
            "/handles/handle-availability/check",
            "fixed-source",
        ))
        .await
        .expect("first request should be infallible");
    let second = service
        .ready()
        .await
        .expect("service should become ready")
        .call(request_with_principal(
            "/handles/handle-availability/check",
            "fixed-source",
        ))
        .await
        .expect("second request should be infallible");

    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn route_visibility_shared_rate_limit_scope_reuses_one_bucket_across_sources() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![
        HttpRouteVisibilityRule::exact(
            HttpRoutePrefix::new("/handles/handle-availability/check").expect("valid route prefix"),
            HttpRouteVisibility::PublicOnly,
        )
        .with_rate_limit_tier(Some(
            HttpRateLimitTierName::new("handle-check-public").expect("valid tier"),
        )),
    ])
    .expect("unique route rule");
    let app = Router::new().route(
        "/handles/handle-availability/check",
        get(|| async { StatusCode::OK }),
    );
    let policies = Arc::new(vec![(
        HttpRateLimitTierName::new("handle-check-public").expect("valid tier"),
        crate::runtime::HttpRateLimitTierPolicy::new(0, 1, 100)
            .expect("valid rate-limit tier policy")
            .with_scope(crate::runtime::HttpRateLimitScope::Shared),
    )]);
    let mut service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener name"),
        HttpListenerVisibility::Public,
        None,
        policies,
        &policy,
    )
    .layer(app);

    let first = service
        .ready()
        .await
        .expect("service should become ready")
        .call(request_with_principal(
            "/handles/handle-availability/check",
            "source-a",
        ))
        .await
        .expect("first request should be infallible");
    let second = service
        .ready()
        .await
        .expect("service should become ready")
        .call(request_with_principal(
            "/handles/handle-availability/check",
            "source-b",
        ))
        .await
        .expect("second request should be infallible");

    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn public_listener_allows_public_route() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![HttpRouteVisibilityRule::new(
        HttpRoutePrefix::new("/public").expect("valid route prefix"),
        HttpRouteVisibility::PublicOnly,
    )])
    .expect("unique route rule");
    let app = Router::new().route("/public/status", get(|| async { StatusCode::OK }));
    let mut service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener name"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/public/status")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn public_listener_rejects_private_only_connect_rpc_path_before_handler_runs() {
    let handler_executed = Arc::new(AtomicBool::new(false));
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![HttpRouteVisibilityRule::exact(
        HttpRoutePrefix::new("/reallyme.api.v1.PrivateService/GetStatus")
            .expect("valid Connect RPC path"),
        HttpRouteVisibility::PrivateOnly,
    )])
    .expect("unique route rule");
    let app = Router::new()
        .route(
            "/reallyme.api.v1.PrivateService/GetStatus",
            get(
                |State(handler_executed): State<Arc<AtomicBool>>| async move {
                    handler_executed.store(true, Ordering::SeqCst);
                    StatusCode::OK
                },
            ),
        )
        .with_state(handler_executed.clone());
    let mut service = route_visibility_layer(
        HttpListenerName::new("public").expect("valid listener name"),
        HttpListenerVisibility::Public,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/reallyme.api.v1.PrivateService/GetStatus")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(!handler_executed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn private_listener_allows_private_only_grpc_method_path() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![HttpRouteVisibilityRule::exact(
        HttpRoutePrefix::new("/reallyme.api.v1.PrivateGrpc/GetStatus")
            .expect("valid gRPC method path"),
        HttpRouteVisibility::PrivateOnly,
    )])
    .expect("unique route rule");
    let app = Router::new().route(
        "/reallyme.api.v1.PrivateGrpc/GetStatus",
        get(|| async { StatusCode::OK }),
    );
    let mut service = route_visibility_layer(
        HttpListenerName::new("private").expect("valid listener name"),
        HttpListenerVisibility::Private,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/reallyme.api.v1.PrivateGrpc/GetStatus")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn both_visibility_allows_public_and_private_listeners() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![HttpRouteVisibilityRule::new(
        HttpRoutePrefix::new("/shared").expect("valid route prefix"),
        HttpRouteVisibility::PublicAndPrivate,
    )])
    .expect("unique route rule");

    for visibility in [
        HttpListenerVisibility::Public,
        HttpListenerVisibility::Private,
    ] {
        let app = Router::new().route("/shared/status", get(|| async { StatusCode::OK }));
        let mut service = route_visibility_layer(
            HttpListenerName::new("listener").expect("valid listener name"),
            visibility,
            None,
            Arc::new(Vec::new()),
            &policy,
        )
        .layer(app);

        let response = service
            .ready()
            .await
            .expect("service should become ready")
            .call(
                Request::builder()
                    .uri("/shared/status")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("route visibility response should be infallible");

        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn public_and_private_visibility_rejects_internal_listener() {
    let handler_executed = Arc::new(AtomicBool::new(false));
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![HttpRouteVisibilityRule::new(
        HttpRoutePrefix::new("/shared").expect("valid route prefix"),
        HttpRouteVisibility::PublicAndPrivate,
    )])
    .expect("unique route rule");
    let app = Router::new()
        .route(
            "/shared/status",
            get(
                |State(handler_executed): State<Arc<AtomicBool>>| async move {
                    handler_executed.store(true, Ordering::SeqCst);
                    StatusCode::OK
                },
            ),
        )
        .with_state(handler_executed.clone());
    let mut service = route_visibility_layer(
        HttpListenerName::new("internal").expect("valid listener name"),
        HttpListenerVisibility::Internal,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/shared/status")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(!handler_executed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn all_visibility_allows_internal_listener() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![HttpRouteVisibilityRule::new(
        HttpRoutePrefix::new("/all").expect("valid route prefix"),
        HttpRouteVisibility::All,
    )])
    .expect("unique route rule");
    let app = Router::new().route("/all/status", get(|| async { StatusCode::OK }));
    let mut service = route_visibility_layer(
        HttpListenerName::new("internal").expect("valid listener name"),
        HttpListenerVisibility::Internal,
        None,
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/all/status")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn route_visibility_layer_applies_listener_rate_limit_tier_when_rule_has_no_tier() {
    let policy = HttpRouteVisibilityPolicy::allow_all();
    let app = Router::new().route(
        "/listening-tier",
        get(|request: Request<Body>| async move {
            if request
                .extensions()
                .get::<HttpRateLimitTierName>()
                .is_some_and(|rate_limit_tier| rate_limit_tier.as_str() == "listener")
            {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }),
    );
    let mut service = route_visibility_layer(
        HttpListenerName::new("public-ingress").expect("valid listener name"),
        HttpListenerVisibility::Public,
        Some(HttpRateLimitTierName::new("listener").expect("valid tier")),
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/listening-tier")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn route_visibility_layer_prefers_route_rate_limit_tier_over_listener_default() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![
        HttpRouteVisibilityRule::new(
            HttpRoutePrefix::new("/admin").expect("valid route prefix"),
            HttpRouteVisibility::All,
        )
        .with_rate_limit_tier(Some(
            HttpRateLimitTierName::new("route").expect("valid tier"),
        )),
    ])
    .expect("unique route rule");
    let app = Router::new().route(
        "/admin/health",
        get(|request: Request<Body>| async move {
            if request
                .extensions()
                .get::<HttpRateLimitTierName>()
                .is_some_and(|rate_limit_tier| rate_limit_tier.as_str() == "route")
            {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }),
    );
    let mut service = route_visibility_layer(
        HttpListenerName::new("admin").expect("valid listener name"),
        HttpListenerVisibility::Private,
        Some(HttpRateLimitTierName::new("listener").expect("valid tier")),
        Arc::new(Vec::new()),
        &policy,
    )
    .layer(app);

    let response = service
        .ready()
        .await
        .expect("service should become ready")
        .call(
            Request::builder()
                .uri("/admin/health")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("route visibility response should be infallible");

    assert_eq!(response.status(), StatusCode::OK);
}
