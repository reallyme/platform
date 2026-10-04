// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use futures_util::StreamExt;
use tonic::Request;
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_server::Health;

use super::RuntimeHealthService;

#[tokio::test]
async fn unknown_service_watch_starts_with_unknown_and_remains_open() {
    let (reporter, _health) = tonic_health::server::health_reporter();
    let service = RuntimeHealthService::new(
        tonic_health::server::HealthService::from_health_reporter(reporter),
    );
    let mut response = service
        .watch(Request::new(HealthCheckRequest {
            service: "unknown.service.v1.Service".to_owned(),
        }))
        .await
        .expect("unknown watch should remain open")
        .into_inner();

    let first = response
        .next()
        .await
        .expect("unknown status should be emitted")
        .expect("unknown status should be successful");
    assert_eq!(first.status, ServingStatus::ServiceUnknown as i32);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), response.next())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn unknown_service_check_is_not_found() {
    let (reporter, _health) = tonic_health::server::health_reporter();
    let service = RuntimeHealthService::new(
        tonic_health::server::HealthService::from_health_reporter(reporter),
    );
    let result = service
        .check(Request::new(HealthCheckRequest {
            service: "unknown.service.v1.Service".to_owned(),
        }))
        .await;
    assert_eq!(
        result.expect_err("unknown check should fail").code(),
        tonic::Code::NotFound
    );
}
