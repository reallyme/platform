// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::liveness::LivenessState;
use super::readiness::Readiness;
use super::response::HealthResponse;

/// Returns the current liveness probe response.
///
/// Liveness is intentionally process-local and must not depend on downstream
/// services or external systems.
pub fn liveness_check() -> HealthResponse {
    HealthResponse::for_liveness(LivenessState::Live)
}

/// Returns the current readiness probe response.
///
/// Readiness reflects whether the service should currently receive traffic. It
/// is driven by explicit state transitions such as successful startup and
/// graceful shutdown.
pub fn readiness_check(readiness: &Readiness) -> HealthResponse {
    HealthResponse::for_readiness(readiness.state())
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
