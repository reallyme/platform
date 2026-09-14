// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use serde_json::json;

use super::JsonErrorResponse;
use crate::http::{ErrorCode, RequestId, TestServer};

async fn bad_request_route() -> JsonErrorResponse {
    JsonErrorResponse::bad_request()
}

#[test]
fn json_error_response_uses_stable_error_envelope() {
    let response = JsonErrorResponse::bad_request();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.body().error().code(), ErrorCode::BadRequest);
    assert_eq!(response.body().error().message(), "Bad request");
    assert_eq!(response.body().error().request_id(), None);
}

#[test]
fn json_error_response_includes_request_id_when_present() {
    let request_id = RequestId::generate();
    let response = JsonErrorResponse::request_timeout().with_request_id(request_id);

    assert_eq!(response.body().error().request_id(), Some(request_id));
}

#[tokio::test]
async fn json_error_response_round_trips_over_http() {
    let app = Router::new().route("/bad-request", get(bad_request_route));
    let server = TestServer::new(app);

    let response = server.get("/bad-request").await;

    response.assert_status_bad_request();
    response.assert_json(&json!({
        "error": {
            "code": "bad_request",
            "message": "Bad request"
        }
    }));
}

#[test]
fn json_error_response_not_found_is_stable() {
    let response = JsonErrorResponse::not_found();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.body().error().code(), ErrorCode::NotFound);
    assert_eq!(response.body().error().message(), "Not found");
}

#[test]
fn json_error_response_method_not_allowed_is_stable() {
    let response = JsonErrorResponse::method_not_allowed();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.body().error().code(), ErrorCode::MethodNotAllowed);
    assert_eq!(response.body().error().message(), "Method not allowed");
}
