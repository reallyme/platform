// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::error::Error as StdError;
use tonic::Code;
use tonic::service::Interceptor;

use super::{
    GrpcAuthenticationInterceptor, GrpcAuthenticationPolicy, GrpcAuthorizationInterceptor,
    GrpcAuthorizationPolicy, GrpcCorrelationInterceptor,
};
use crate::grpc::{GrpcStatusCode, StaticGrpcStatus, ToGrpcStatus};
use std::fmt;

#[derive(Debug, Clone, Copy)]
struct RejectUnauthenticated;

impl fmt::Display for RejectUnauthenticated {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unauthenticated")
    }
}

impl StdError for RejectUnauthenticated {}

impl ToGrpcStatus for RejectUnauthenticated {
    fn to_grpc_status(&self) -> tonic::Status {
        StaticGrpcStatus::new(GrpcStatusCode::Unauthenticated, "unauthenticated").to_grpc_status()
    }
}

struct AlwaysRejectAuthentication;

impl GrpcAuthenticationPolicy for AlwaysRejectAuthentication {
    type Error = RejectUnauthenticated;

    fn authenticate(&self, _metadata: &tonic::metadata::MetadataMap) -> Result<(), Self::Error> {
        Err(RejectUnauthenticated)
    }
}

#[derive(Debug, Clone, Copy)]
struct RejectPermissionDenied;

impl fmt::Display for RejectPermissionDenied {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("permission denied")
    }
}

impl StdError for RejectPermissionDenied {}

impl ToGrpcStatus for RejectPermissionDenied {
    fn to_grpc_status(&self) -> tonic::Status {
        StaticGrpcStatus::new(GrpcStatusCode::PermissionDenied, "permission denied")
            .to_grpc_status()
    }
}

struct AlwaysRejectAuthorization;

impl GrpcAuthorizationPolicy for AlwaysRejectAuthorization {
    type Error = RejectPermissionDenied;

    fn authorize(&self, _metadata: &tonic::metadata::MetadataMap) -> Result<(), Self::Error> {
        Err(RejectPermissionDenied)
    }
}

#[test]
fn correlation_interceptor_attaches_correlation_ids() {
    let mut interceptor = GrpcCorrelationInterceptor;
    let request = interceptor
        .call(tonic::Request::new(()))
        .expect("correlation interceptor should normalize request metadata");

    assert!(request.metadata().get("x-request-id").is_some());
    assert!(request.metadata().get("x-trace-id").is_some());
    assert!(
        request
            .extensions()
            .get::<crate::grpc::GrpcCorrelationIds>()
            .is_some()
    );
}

#[test]
fn authentication_interceptor_maps_typed_status() {
    let mut interceptor = GrpcAuthenticationInterceptor::new(AlwaysRejectAuthentication);

    let status = interceptor
        .call(tonic::Request::new(()))
        .expect_err("authentication should be rejected");

    assert_eq!(status.code(), Code::Unauthenticated);
    assert_eq!(status.message(), "unauthenticated");
}

#[test]
fn authorization_interceptor_maps_typed_status() {
    let mut interceptor = GrpcAuthorizationInterceptor::new(AlwaysRejectAuthorization);

    let status = interceptor
        .call(tonic::Request::new(()))
        .expect_err("authorization should be rejected");

    assert_eq!(status.code(), Code::PermissionDenied);
    assert_eq!(status.message(), "permission denied");
}
