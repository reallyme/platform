// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{GrpcHealthServingStatus, grpc_health_serving_status};
use crate::health::{HealthResponse, LivenessState, ReadinessState};

#[test]
fn serving_status_maps_from_health_response() {
    let liveness = HealthResponse::for_liveness(LivenessState::Live);
    let readiness = HealthResponse::for_readiness(ReadinessState::NotReady);

    assert_eq!(
        grpc_health_serving_status(liveness),
        GrpcHealthServingStatus::Serving
    );
    assert_eq!(
        grpc_health_serving_status(readiness),
        GrpcHealthServingStatus::NotServing
    );
}
