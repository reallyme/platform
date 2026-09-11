// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use tonic::server::NamedService;
pub use tonic_health::ServingStatus as GrpcHealthServingStatus;
pub use tonic_health::server::{HealthReporter as GrpcHealthReporter, health_reporter};

use crate::health::{HealthResponse, Readiness, readiness_check};

/// Maps a stable health response into the standard gRPC health serving status.
pub fn grpc_health_serving_status(response: HealthResponse) -> GrpcHealthServingStatus {
    match response.grpc_serving_status() {
        crate::health::GrpcServingStatus::Serving => GrpcHealthServingStatus::Serving,
        crate::health::GrpcServingStatus::NotServing => GrpcHealthServingStatus::NotServing,
    }
}

/// Applies a stable health response to a named gRPC health reporter service.
pub async fn set_named_service_status<Service>(
    reporter: &mut GrpcHealthReporter,
    response: HealthResponse,
) where
    Service: NamedService,
{
    match grpc_health_serving_status(response) {
        GrpcHealthServingStatus::Serving => reporter.set_serving::<Service>().await,
        GrpcHealthServingStatus::NotServing | GrpcHealthServingStatus::Unknown => {
            reporter.set_not_serving::<Service>().await
        }
    }
}

/// Applies the current readiness state to a named gRPC health reporter service.
pub async fn set_readiness_status<Service>(reporter: &mut GrpcHealthReporter, readiness: &Readiness)
where
    Service: NamedService,
{
    set_named_service_status::<Service>(reporter, readiness_check(readiness)).await;
}

#[cfg(test)]
mod tests {
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
}
