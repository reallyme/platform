// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::grpc_health_status_for_readiness;
use crate::grpc::GrpcHealthServingStatus;
use crate::health::Readiness;

#[test]
fn grpc_health_tracks_current_readiness_state() {
    let readiness = Readiness::new();

    assert_eq!(
        grpc_health_status_for_readiness(&readiness),
        GrpcHealthServingStatus::NotServing
    );

    readiness.mark_ready();

    assert_eq!(
        grpc_health_status_for_readiness(&readiness),
        GrpcHealthServingStatus::Serving
    );

    readiness.mark_not_ready();

    assert_eq!(
        grpc_health_status_for_readiness(&readiness),
        GrpcHealthServingStatus::NotServing
    );
}
