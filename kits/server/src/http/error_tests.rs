// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::StatusCode;

use super::{ErrorCode, PublicHttpError, ToHttpErrorResponse};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestError {
    InvalidPayload,
}

impl ToHttpErrorResponse for TestError {
    fn public_http_error(&self) -> PublicHttpError {
        match self {
            Self::InvalidPayload => PublicHttpError::from_code(ErrorCode::BadRequest),
        }
    }
}

#[test]
fn public_http_error_uses_stable_status_and_message_mapping() {
    let error = PublicHttpError::from_code(ErrorCode::RequestTimeout);

    assert_eq!(error.status(), StatusCode::REQUEST_TIMEOUT);
    assert_eq!(error.code(), ErrorCode::RequestTimeout);
    assert_eq!(
        ErrorCode::RequestTimeout.public_message(),
        "Request timeout"
    );
}

#[test]
fn typed_http_error_trait_maps_to_stable_public_error() {
    let response = TestError::InvalidPayload.to_http_error_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.body().error().code(), ErrorCode::BadRequest);
}
