// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use axum::Json;
    use axum::Router;
    use axum::extract::State;
    use axum::routing::get;
    use serde_json::json;

    use super::{liveness_check, readiness_check};
    use crate::health::{Readiness, ReadinessState};
    use crate::http::TestServer;

    async fn healthz_route() -> (axum::http::StatusCode, Json<crate::health::HealthResponse>) {
        let response = liveness_check();
        (response.http_status_code(), Json(response))
    }

    async fn readyz_route(
        State(readiness): State<Readiness>,
    ) -> (axum::http::StatusCode, Json<crate::health::HealthResponse>) {
        let response = readiness_check(&readiness);
        (response.http_status_code(), Json(response))
    }

    #[test]
    fn readiness_check_uses_current_readiness_state() {
        let readiness = Readiness::new();
        let initial = readiness_check(&readiness);

        assert_eq!(initial.readiness, Some(ReadinessState::NotReady));

        readiness.mark_ready();
        let ready = readiness_check(&readiness);

        assert_eq!(ready.readiness, Some(ReadinessState::Ready));
    }

    #[tokio::test]
    async fn healthz_and_readyz_map_to_expected_http_statuses() {
        let readiness = Readiness::new();
        let app = Router::new()
            .route("/healthz", get(healthz_route))
            .route("/readyz", get(readyz_route))
            .with_state(readiness.clone());
        let server = TestServer::new(app);

        let healthz = server.get("/healthz").await;
        healthz.assert_status_ok();
        healthz.assert_json(&json!({
            "kind": "liveness",
            "status": "serving",
            "liveness": "live"
        }));

        let readyz_not_ready = server.get("/readyz").await;
        readyz_not_ready.assert_status_service_unavailable();
        readyz_not_ready.assert_json(&json!({
            "kind": "readiness",
            "status": "not_serving",
            "readiness": "not_ready"
        }));

        readiness.mark_ready();

        let readyz_ready = server.get("/readyz").await;
        readyz_ready.assert_status_ok();
        readyz_ready.assert_json(&json!({
            "kind": "readiness",
            "status": "serving",
            "readiness": "ready"
        }));
    }
}
