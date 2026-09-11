// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::Infallible;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use tower::{Layer, Service};

use crate::http::{HttpListenerIdentity, HttpListenerName, HttpListenerVisibility};

/// Creates middleware that attaches the runtime-selected listener identity.
///
/// The identity comes from the concrete socket/listener configuration, never
/// from client-controlled forwarding headers.
pub fn listener_identity_layer(
    listener_name: HttpListenerName,
    listener_visibility: HttpListenerVisibility,
    local_socket_addr: std::net::SocketAddr,
) -> ListenerIdentityLayer {
    ListenerIdentityLayer {
        listener_name,
        listener_visibility,
        local_socket_addr,
    }
}

/// Layer that attaches listener identity before downstream middleware runs.
#[derive(Debug, Clone)]
pub struct ListenerIdentityLayer {
    listener_name: HttpListenerName,
    listener_visibility: HttpListenerVisibility,
    local_socket_addr: std::net::SocketAddr,
}

impl<S> Layer<S> for ListenerIdentityLayer {
    type Service = ListenerIdentityService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ListenerIdentityService {
            inner,
            listener_name: self.listener_name.clone(),
            listener_visibility: self.listener_visibility.clone(),
            local_socket_addr: self.local_socket_addr,
        }
    }
}

/// Service that attaches runtime listener identity to request extensions.
#[derive(Clone)]
pub struct ListenerIdentityService<S> {
    inner: S,
    listener_name: HttpListenerName,
    listener_visibility: HttpListenerVisibility,
    local_socket_addr: std::net::SocketAddr,
}

impl<S> Service<Request<Body>> for ListenerIdentityService<S>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        request.extensions_mut().insert(HttpListenerIdentity::new(
            self.listener_name.clone(),
            self.listener_visibility.clone(),
            Some(self.local_socket_addr),
        ));

        self.inner.call(request)
    }
}

pub(crate) fn listener_name_for_request(request: &Request<Body>) -> &HttpListenerName {
    match request.extensions().get::<HttpListenerIdentity>() {
        Some(identity) => identity.name(),
        None => HttpListenerName::unknown_ref(),
    }
}

#[cfg(test)]
mod tests {
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
                Some(identity) if identity.local_socket_addr() == Some(local_socket) => {
                    StatusCode::OK
                }
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
}
