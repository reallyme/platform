// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::{Layer, Service, ServiceExt};

use crate::http::{HttpListenerIdentity, HttpListenerName, HttpListenerVisibility};

use super::listener_identity_layer;

#[tokio::test]
async fn listener_identity_layer_attaches_local_socket_addr() {
    let local_socket = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 3025));
    let app = tower::service_fn(move |request: Request<Body>| async move {
        let status = match request.extensions().get::<HttpListenerIdentity>() {
            Some(identity) if identity.local_socket_addr() == Some(local_socket) => StatusCode::OK,
            Some(_) | None => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let mut response = axum::response::Response::new(Body::empty());
        *response.status_mut() = status;
        Ok::<_, std::convert::Infallible>(response)
    });
    let mut service = listener_identity_layer(
        HttpListenerName::new("public").expect("valid listener name"),
        HttpListenerVisibility::Public,
        local_socket,
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
        .expect("listener identity response should be infallible");

    assert_eq!(response.status(), StatusCode::OK);
}
