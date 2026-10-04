// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app_adapter::ExampleWorkerMethod;
use crate::error::{
    ExampleWorkerHostError, WorkerConnectErrorCode, WorkerPublicErrorCode,
    classify_worker_connect_error, classify_worker_error,
};
use crate::model::{WorkerErrorBody, WorkerErrorEnvelope, WorkerHelloResponse};
use crate::response::WorkerRouteResponse;
use crate::routing::{
    allowed_method_for_path, allowed_origin, is_connect_content_type, is_known_app_path,
    is_known_operational_path, is_valid_hello_request, parse_connect_timeout_values,
    route_example_app, route_example_app_with_deadline, valid_operational_probe_token,
};
use example_app::app::{ExampleAppConfig, new_context};
use example_app::ports::ExamplePorts;
use serde::Deserialize;

fn valid_operational_probe_headers(headers: &[String], expected: &str) -> bool {
    let [authorization] = headers else {
        return false;
    };
    valid_operational_probe_token(authorization, expected)
}

#[derive(Deserialize)]
struct HostParityCase {
    enabled: bool,
    transport: HostParityTransport,
    expected_status: u16,
    expected_code: Option<String>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HostParityTransport {
    HttpHello,
    ConnectHello,
}

#[test]
fn worker_matches_shared_host_parity_cases() {
    let cases: Vec<HostParityCase> =
        serde_json::from_str(include_str!("../../../conformance/host-parity/cases.json"))
            .expect("valid shared parity cases");
    for case in cases {
        let context = new_context(
            ExampleAppConfig::new(case.enabled),
            ExamplePorts::unconfigured(),
        );
        let (method, path) = match case.transport {
            HostParityTransport::HttpHello => (ExampleWorkerMethod::Get, "/hello"),
            HostParityTransport::ConnectHello => (
                ExampleWorkerMethod::Post,
                example_app::adapters::connect::EXAMPLE_HELLO_CONNECT_RPC_PATH,
            ),
        };
        let routed = route_example_app(&context, method, path);
        let (status, code) = match (case.transport, routed) {
            (HostParityTransport::HttpHello, Ok(Some(WorkerRouteResponse::PlainHello(body))))
            | (
                HostParityTransport::ConnectHello,
                Ok(Some(WorkerRouteResponse::ConnectHello(body))),
            ) => {
                assert_eq!(body, "hello from example-app");
                (200, None)
            }
            (HostParityTransport::HttpHello, Err(error)) => {
                let (code, _message, status) = classify_worker_error(error);
                (
                    status,
                    Some(serde_json::to_value(code).expect("public code serializes")),
                )
            }
            (HostParityTransport::ConnectHello, Err(error)) => {
                let (code, _message, status) = classify_worker_connect_error(error);
                (
                    status,
                    Some(serde_json::to_value(code).expect("connect code serializes")),
                )
            }
            _ => unreachable!("shared parity case did not reach its intended route"),
        };
        assert_eq!(status, case.expected_status);
        assert_eq!(code, case.expected_code.map(serde_json::Value::String));
    }
}

fn test_context() -> example_app::app::ExampleAppContext {
    new_context(ExampleAppConfig::new(true), ExamplePorts::unconfigured())
}

#[test]
fn worker_host_routes_hello_to_example_app_core() {
    let response = route_example_app(&test_context(), ExampleWorkerMethod::Get, "/hello")
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
        &test_context(),
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
fn worker_connect_boundary_rejects_wrong_content_types_and_malformed_protobuf() {
    assert!(is_connect_content_type("application/proto"));
    assert!(is_connect_content_type("APPLICATION/PROTO"));
    assert!(!is_connect_content_type("text/plain"));
    assert!(!is_connect_content_type("application/json"));

    assert!(is_valid_hello_request(&[]));
    assert!(!is_valid_hello_request(&[0x0a, 0x02, 0xff]));
}

#[test]
fn worker_connect_timeout_is_validated_and_reaches_app_core() {
    use std::time::Duration;

    assert_eq!(parse_connect_timeout_values(&[]), Ok(None));
    assert_eq!(
        parse_connect_timeout_values(&["1200".to_owned()]),
        Ok(Some(Duration::from_millis(1200)))
    );
    for values in [
        vec![String::new()],
        vec!["+1".to_owned()],
        vec!["1S".to_owned()],
        vec!["18446744073709551616".to_owned()],
        vec!["1".to_owned(), "2".to_owned()],
    ] {
        assert!(parse_connect_timeout_values(&values).is_err());
    }
    let error = route_example_app_with_deadline(
        &test_context(),
        ExampleWorkerMethod::Post,
        example_app::adapters::connect::EXAMPLE_HELLO_CONNECT_RPC_PATH,
        Some(Duration::ZERO),
    )
    .expect_err("expired deadline must prevent the app call");
    assert_eq!(
        classify_worker_connect_error(error).0,
        WorkerConnectErrorCode::DeadlineExceeded
    );
}

#[test]
fn worker_host_does_not_route_unknown_app_paths() {
    let response = route_example_app(&test_context(), ExampleWorkerMethod::Get, "/unknown")
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
    assert!(!is_known_operational_path("/version"));
    assert!(!is_known_operational_path("/metrics"));
    assert!(!is_known_operational_path("/internal/stats"));
}

#[test]
fn operational_probe_requires_a_configured_bearer_token() {
    let token = "a".repeat(32);
    let authorization = format!("Bearer {token}");
    assert!(valid_operational_probe_headers(
        std::slice::from_ref(&authorization),
        &token
    ));
    assert!(!valid_operational_probe_headers(&[], &token));
    assert!(!valid_operational_probe_headers(
        &[authorization.clone(), authorization.clone()],
        &token
    ));
    assert!(valid_operational_probe_token(&authorization, &token));
    assert!(!valid_operational_probe_token("", &token));
    assert!(!valid_operational_probe_token(&token, &token));
    assert!(!valid_operational_probe_token(
        &format!("Bearer {}", "b".repeat(32)),
        &token
    ));
    assert!(!valid_operational_probe_token(
        &format!("Bearer {}", "a".repeat(31)),
        &token
    ));
    assert!(!valid_operational_probe_token(
        &format!("Bearer {}", "a".repeat(32)),
        "short"
    ));
    assert!(!valid_operational_probe_token(
        &format!("Bearer {}", "a".repeat(257)),
        &"a".repeat(257)
    ));
    assert_eq!(allowed_method_for_path("/healthz"), None);
    assert_eq!(allowed_method_for_path("/readyz"), None);
}

#[test]
fn worker_cors_origin_and_preflight_method_match_exactly() {
    let allowed = vec!["https://app.reallyme.net".to_owned()];
    assert_eq!(
        allowed_origin(&allowed, "https://app.reallyme.net"),
        Some("https://app.reallyme.net")
    );
    assert_eq!(allowed_origin(&allowed, "https://evil.reallyme.net"), None);
    assert_eq!(
        allowed_origin(&allowed, "https://app.reallyme.net.evil"),
        None
    );
    assert_eq!(allowed_method_for_path("/hello"), Some("GET"));
    assert_eq!(
        allowed_method_for_path(example_app::adapters::connect::EXAMPLE_HELLO_CONNECT_RPC_PATH),
        Some("POST")
    );
    assert_eq!(allowed_method_for_path("/unknown"), None);
}

#[test]
fn worker_configuration_failure_maps_to_internal_http_and_connect_errors() {
    let http = classify_worker_error(ExampleWorkerHostError::InvalidConfiguration);
    let connect = classify_worker_connect_error(ExampleWorkerHostError::InvalidConfiguration);

    assert_eq!(http.0, WorkerPublicErrorCode::InternalServerError);
    assert_eq!(http.2, 500);
    assert_eq!(connect.0, WorkerConnectErrorCode::Internal);
    assert_eq!(connect.2, 500);
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
