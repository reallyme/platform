// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{GrpcServingStatus, HealthCheckKind, HealthResponse, HealthStatus};
use crate::health::{LivenessState, ReadinessState};
#[cfg(feature = "http")]
use axum::http::StatusCode;

#[test]
fn liveness_response_is_always_serving() {
    let response = HealthResponse::for_liveness(LivenessState::Live);

    assert_eq!(response.kind, HealthCheckKind::Liveness);
    assert_eq!(response.status, HealthStatus::Serving);
    #[cfg(feature = "http")]
    assert_eq!(response.http_status_code(), StatusCode::OK);
    assert_eq!(response.grpc_serving_status(), GrpcServingStatus::Serving);
}

#[test]
fn readiness_response_maps_not_ready_to_not_serving() {
    let response = HealthResponse::for_readiness(ReadinessState::NotReady);

    assert_eq!(response.kind, HealthCheckKind::Readiness);
    assert_eq!(response.status, HealthStatus::NotServing);
    #[cfg(feature = "http")]
    assert_eq!(response.http_status_code(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.grpc_serving_status(),
        GrpcServingStatus::NotServing
    );
}
