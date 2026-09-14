// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Router;
use serde_json::json;

use super::super::{HEALTHZ_PATH, READYZ_PATH, healthz_route, readyz_route};
use crate::http::TestServer;

#[tokio::test]
async fn health_and_ready_routes_return_stable_responses() {
    let readiness = crate::health::Readiness::new();
    let app = Router::new()
        .route(HEALTHZ_PATH, healthz_route())
        .route(READYZ_PATH, readyz_route(readiness.clone()));
    let server = TestServer::new(app);

    let healthz = server.get(HEALTHZ_PATH).await;
    healthz.assert_status_ok();
    healthz.assert_json(&json!({
        "kind": "liveness",
        "status": "serving",
        "liveness": "live"
    }));

    let readyz_initial = server.get(READYZ_PATH).await;
    readyz_initial.assert_status_service_unavailable();
    readyz_initial.assert_json(&json!({
        "kind": "readiness",
        "status": "not_serving",
        "readiness": "not_ready"
    }));

    readiness.mark_ready();

    let readyz_ready = server.get(READYZ_PATH).await;
    readyz_ready.assert_status_ok();
    readyz_ready.assert_json(&json!({
        "kind": "readiness",
        "status": "serving",
        "readiness": "ready"
    }));
}
