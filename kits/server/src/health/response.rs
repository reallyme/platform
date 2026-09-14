// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(feature = "http")]
use axum::http::StatusCode;
use serde::Serialize;

use super::liveness::LivenessState;
use super::readiness::ReadinessState;

/// Stable service health status used across transport mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// The service should be treated as serving traffic.
    Serving,
    /// The service should be treated as not serving traffic.
    NotServing,
}

/// Identifies which health surface produced the response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthCheckKind {
    /// Process liveness probe.
    Liveness,
    /// Traffic-readiness probe.
    Readiness,
}

/// Stable response model for health and readiness surfaces.
///
/// The model is transport-neutral and can be reused for HTTP JSON responses,
/// gRPC health mappings, and operator-facing diagnostics. It intentionally
/// carries typed health state rather than free-form strings or ad hoc payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HealthResponse {
    /// Which probe surface produced this response.
    pub kind: HealthCheckKind,
    /// Whether the service should be treated as serving traffic.
    pub status: HealthStatus,
    /// Process liveness information when the response came from a liveness
    /// probe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liveness: Option<LivenessState>,
    /// Traffic-readiness information when the response came from a readiness
    /// probe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness: Option<ReadinessState>,
}

impl HealthResponse {
    /// Creates a stable liveness response.
    pub fn for_liveness(state: LivenessState) -> Self {
        Self {
            kind: HealthCheckKind::Liveness,
            status: HealthStatus::from(state),
            liveness: Some(state),
            readiness: None,
        }
    }

    /// Creates a stable readiness response.
    pub fn for_readiness(state: ReadinessState) -> Self {
        Self {
            kind: HealthCheckKind::Readiness,
            status: HealthStatus::from(state),
            liveness: None,
            readiness: Some(state),
        }
    }

    /// Returns the HTTP status code associated with this health response.
    #[cfg(feature = "http")]
    pub fn http_status_code(self) -> StatusCode {
        match self.status {
            HealthStatus::Serving => StatusCode::OK,
            HealthStatus::NotServing => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Returns the generic gRPC health serving status associated with this
    /// response.
    pub fn grpc_serving_status(self) -> GrpcServingStatus {
        match self.status {
            HealthStatus::Serving => GrpcServingStatus::Serving,
            HealthStatus::NotServing => GrpcServingStatus::NotServing,
        }
    }
}

impl From<LivenessState> for HealthStatus {
    fn from(_value: LivenessState) -> Self {
        Self::Serving
    }
}

impl From<ReadinessState> for HealthStatus {
    fn from(value: ReadinessState) -> Self {
        match value {
            ReadinessState::Ready => Self::Serving,
            ReadinessState::NotReady => Self::NotServing,
        }
    }
}

/// Transport-neutral gRPC health serving status.
///
/// This stays separate from generated protobuf code so the shared server kit
/// does not need to leak generated types through its public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrpcServingStatus {
    /// The service is serving traffic.
    Serving,
    /// The service is not serving traffic.
    NotServing,
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
