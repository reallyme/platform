// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::error::Error as StdError;

use tonic::metadata::MetadataMap;
use tonic::service::Interceptor;
use tonic::{Request, Status};

use super::metadata::attach_correlation_ids;
use super::status::ToGrpcStatus;

/// Synchronous authentication policy for gRPC interceptors.
///
/// Tonic interceptors are synchronous, so this scaffolding is intentionally
/// limited to lightweight transport-bound checks such as API-key headers, mTLS
/// metadata, or other already-available request metadata. Heavier async
/// authentication flows should happen inside service entrypoints or dedicated
/// middleware layers built outside this crate.
///
/// The interceptor type is generic over the policy instead of storing a trait
/// object. This preserves each policy's typed error surface and avoids erasing
/// transport mapping behavior behind `Box<dyn Error>`.
pub trait GrpcAuthenticationPolicy: Send + Sync {
    /// Typed authentication failure emitted by the policy.
    type Error: StdError + ToGrpcStatus + Send + Sync + 'static;

    /// Authenticates the request metadata.
    fn authenticate(&self, metadata: &MetadataMap) -> Result<(), Self::Error>;
}

/// Synchronous authorization policy for gRPC interceptors.
///
/// Like authentication, this remains synchronous because tonic interceptors do
/// not support async execution. It is meant for coarse transport-scoped checks,
/// not business authorization.
///
/// The interceptor type is generic over the policy instead of storing a trait
/// object. Service-specific authorization should keep product permission logic
/// outside server-kit and expose one typed, transport-safe error mapping.
pub trait GrpcAuthorizationPolicy: Send + Sync {
    /// Typed authorization failure emitted by the policy.
    type Error: StdError + ToGrpcStatus + Send + Sync + 'static;

    /// Authorizes the request metadata.
    fn authorize(&self, metadata: &MetadataMap) -> Result<(), Self::Error>;
}

/// Interceptor that normalizes request and trace identifiers on inbound calls.
#[derive(Debug, Default, Clone, Copy)]
pub struct GrpcCorrelationInterceptor;

impl Interceptor for GrpcCorrelationInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        attach_correlation_ids(&mut request).map_err(|error| error.to_grpc_status())?;
        Ok(request)
    }
}

/// Interceptor scaffolding for synchronous gRPC authentication.
pub struct GrpcAuthenticationInterceptor<Policy> {
    policy: Policy,
}

impl<Policy> GrpcAuthenticationInterceptor<Policy> {
    /// Creates a gRPC authentication interceptor from a policy.
    pub fn new(policy: Policy) -> Self {
        Self { policy }
    }
}

impl<Policy> Interceptor for GrpcAuthenticationInterceptor<Policy>
where
    Policy: GrpcAuthenticationPolicy,
{
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        self.policy
            .authenticate(request.metadata())
            .map_err(|error| error.to_grpc_status())?;
        Ok(request)
    }
}

/// Interceptor scaffolding for synchronous gRPC authorization.
pub struct GrpcAuthorizationInterceptor<Policy> {
    policy: Policy,
}

impl<Policy> GrpcAuthorizationInterceptor<Policy> {
    /// Creates a gRPC authorization interceptor from a policy.
    pub fn new(policy: Policy) -> Self {
        Self { policy }
    }
}

impl<Policy> Interceptor for GrpcAuthorizationInterceptor<Policy>
where
    Policy: GrpcAuthorizationPolicy,
{
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        self.policy
            .authorize(request.metadata())
            .map_err(|error| error.to_grpc_status())?;
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
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
            StaticGrpcStatus::new(GrpcStatusCode::Unauthenticated, "unauthenticated")
                .to_grpc_status()
        }
    }

    struct AlwaysRejectAuthentication;

    impl GrpcAuthenticationPolicy for AlwaysRejectAuthentication {
        type Error = RejectUnauthenticated;

        fn authenticate(
            &self,
            _metadata: &tonic::metadata::MetadataMap,
        ) -> Result<(), Self::Error> {
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
}
