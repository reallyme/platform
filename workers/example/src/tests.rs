// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app_adapter::ExampleWorkerMethod;
use crate::error::{ExampleWorkerHostError, WorkerPublicErrorCode};
use crate::model::{WorkerErrorBody, WorkerErrorEnvelope, WorkerHelloResponse};
use crate::response::WorkerRouteResponse;
use crate::routing::{is_known_app_path, is_known_operational_path, route_example_app};

#[test]
fn worker_host_routes_hello_to_example_app_core() {
    let response = route_example_app(ExampleWorkerMethod::Get, "/hello")
        .expect("checked-in config should be valid")
        .expect("hello route should match");

    assert_eq!(
        response,
        WorkerRouteResponse::PlainHello("hello from example-app")
    );
}

#[test]
fn worker_host_routes_connect_path_to_example_app_core() {
    let response = route_example_app(
        ExampleWorkerMethod::Post,
        example_app::adapters::connect::EXAMPLE_HELLO_CONNECT_RPC_PATH,
    )
    .expect("checked-in config should be valid")
    .expect("connect route should match");

    assert_eq!(
        response,
        WorkerRouteResponse::ConnectHello("hello from example-app")
    );
}

#[test]
fn worker_host_does_not_route_unknown_app_paths() {
    let response = route_example_app(ExampleWorkerMethod::Get, "/unknown")
        .expect("checked-in config should be valid");

    assert!(response.is_none());
}

#[test]
fn worker_host_distinguishes_unknown_paths_from_method_mismatch() {
    assert!(is_known_app_path("/hello"));
    assert!(is_known_app_path(
        example_app::adapters::connect::EXAMPLE_HELLO_CONNECT_RPC_PATH
    ));
    assert!(!is_known_app_path("/unknown"));
}

#[test]
fn worker_operational_routes_are_known_for_method_mismatch_mapping() {
    assert!(is_known_operational_path("/healthz"));
    assert!(is_known_operational_path("/readyz"));
    assert!(is_known_operational_path("/version"));
    assert!(is_known_operational_path("/metrics"));
    assert!(!is_known_operational_path("/internal/stats"));
}

#[test]
fn worker_host_error_kinds_are_typed() {
    assert_eq!(
        ExampleWorkerHostError::AppUnavailable,
        ExampleWorkerHostError::AppUnavailable,
    );
}

#[test]
fn worker_host_hello_response_is_json_dto() {
    let encoded = serde_json::to_value(WorkerHelloResponse {
        message: "hello from example-app",
    })
    .expect("worker hello response should serialize");

    assert_eq!(
        encoded,
        serde_json::json!({
            "message": "hello from example-app"
        })
    );
}

#[test]
fn worker_host_error_response_shape_is_stable() {
    let encoded = serde_json::to_value(WorkerErrorEnvelope {
        error: WorkerErrorBody {
            code: WorkerPublicErrorCode::NotFound,
            message: "Not found",
        },
    })
    .expect("worker error envelope should serialize");

    assert_eq!(
        encoded,
        serde_json::json!({
            "error": {
                "code": "not_found",
                "message": "Not found"
            }
        })
    );
}
