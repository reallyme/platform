// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! gRPC transport conventions and infrastructure helpers.
//!
//! The gRPC module is intentionally transport-focused. It provides stable
//! status mapping, correlation-ID propagation, deadline helpers, interceptor
//! scaffolding, and optional health/reflection support without introducing any
//! product-specific protobuf services or business RPC logic.

mod deadline;
mod error;
mod health;
mod interceptors;
mod message_size;
mod metadata;
mod policy;
mod reflection;
mod status;

pub use deadline::{
    GRPC_TIMEOUT_METADATA_KEY, GrpcTimeout, apply_client_timeout, caller_timeout,
    effective_timeout, run_with_timeout,
};
pub use error::{
    GrpcDeadlineConfigField, GrpcDeadlineError, GrpcDeadlineErrorReason, GrpcDeadlineMetadataField,
    GrpcMetadataError, GrpcMetadataErrorReason, GrpcMetadataField, GrpcReflectionError,
};
pub use health::{
    GrpcHealthReporter, GrpcHealthServingStatus, grpc_health_serving_status, health_reporter,
    set_named_service_status, set_readiness_status,
};
pub use interceptors::{
    GrpcAuthenticationInterceptor, GrpcAuthenticationPolicy, GrpcAuthorizationInterceptor,
    GrpcAuthorizationPolicy, GrpcCorrelationInterceptor,
};
pub use message_size::{
    DEFAULT_GRPC_DECODING_MESSAGE_SIZE_BYTES, DEFAULT_GRPC_ENCODING_MESSAGE_SIZE_BYTES,
};
pub use metadata::{
    GRPC_REQUEST_ID_METADATA_KEY, GRPC_TRACE_ID_METADATA_KEY, GrpcCorrelationIds,
    attach_correlation_ids, attach_correlation_ids_to_status, correlation_ids_from_request,
    request_id_from_metadata, request_id_from_request, trace_id_from_metadata,
    trace_id_from_request,
};
pub use policy::{GrpcPolicy, grpc_policy_layer};
pub use reflection::{GrpcReflectionMode, ReflectionServiceBuilder, reflection_builder};
pub use status::{GrpcStatusCode, StaticGrpcStatus, ToGrpcStatus};
