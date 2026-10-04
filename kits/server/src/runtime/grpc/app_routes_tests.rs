// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{GrpcAppRoutes, GrpcAppRoutesErrorReason, valid_service_name};
use std::convert::Infallible;
use std::task::{Context, Poll};
use tonic::body::Body;
use tonic::codegen::http::Request;
use tonic::server::NamedService;
use tower::Service;

#[derive(Clone)]
struct ExampleService<S>(S);

impl<S> NamedService for ExampleService<S> {
    const NAME: &'static str = "reallyme.example.v1.ExampleService";
}

impl<S> Service<Request<Body>> for ExampleService<S>
where
    S: Service<Request<Body>, Error = Infallible>,
{
    type Response = S::Response;
    type Error = Infallible;
    type Future = S::Future;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(context)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        self.0.call(request)
    }
}

#[test]
fn service_names_follow_protobuf_identifiers() {
    assert!(valid_service_name("reallyme.example.v1.ExampleService"));
    assert!(valid_service_name("_internal.Service_2"));
    for name in [
        "",
        ".Service",
        "example..Service",
        "example.Service.",
        "a-b.C",
        "1.Service",
    ] {
        assert!(!valid_service_name(name));
    }
    assert!(!valid_service_name(&"A".repeat(256)));
}

#[test]
fn runtime_owned_health_service_cannot_be_registered_twice() {
    let (_reporter, service) = tonic_health::server::health_reporter();
    let result = GrpcAppRoutes::new(service);
    assert!(matches!(
        result,
        Err(error) if error.reason() == GrpcAppRoutesErrorReason::ReservedServiceName
    ));
}

#[test]
fn route_composition_tracks_names_and_rejects_duplicates() {
    let (_reporter, service) = tonic_health::server::health_reporter();
    let routes =
        GrpcAppRoutes::new(ExampleService(service)).expect("valid app service should register");
    let (_reporter, duplicate) = tonic_health::server::health_reporter();
    let result = routes.add_service(ExampleService(duplicate));
    assert!(matches!(
        result,
        Err(error) if error.reason() == GrpcAppRoutesErrorReason::DuplicateServiceName
    ));

    let (_reporter, service) = tonic_health::server::health_reporter();
    let (_routes, names) = GrpcAppRoutes::new(ExampleService(service))
        .expect("valid app service should register")
        .into_parts();
    assert_eq!(names, ["reallyme.example.v1.ExampleService"]);
}
