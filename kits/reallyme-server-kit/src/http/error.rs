// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::StatusCode;
use serde::Serialize;

use super::response::JsonErrorResponse;

/// Stable error codes exposed by infrastructure endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request payload or parameters are invalid.
    BadRequest,
    /// Authentication is required or invalid.
    Unauthorized,
    /// The caller is authenticated but not permitted.
    Forbidden,
    /// The requested resource does not exist.
    NotFound,
    /// The HTTP method is not allowed for this route.
    MethodNotAllowed,
    /// The request conflicts with current resource state.
    Conflict,
    /// The request body exceeds the configured limit.
    PayloadTooLarge,
    /// The request headers exceed configured limits.
    RequestHeaderFieldsTooLarge,
    /// The request exceeded the configured timeout.
    RequestTimeout,
    /// The request exceeded a configured rate limit.
    TooManyRequests,
    /// The server is temporarily unable to accept more in-flight work.
    ServiceUnavailable,
    /// The caller sent an unsupported content type.
    UnsupportedMediaType,
    /// An unexpected server-side failure occurred.
    InternalServerError,
}

impl ErrorCode {
    /// Returns the stable HTTP status associated with this public error code.
    pub const fn http_status(self) -> StatusCode {
        match self {
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::Conflict => StatusCode::CONFLICT,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::RequestHeaderFieldsTooLarge => StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            Self::RequestTimeout => StatusCode::REQUEST_TIMEOUT,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Returns the stable public message associated with this error code.
    pub const fn public_message(self) -> &'static str {
        match self {
            Self::BadRequest => "Bad request",
            Self::Unauthorized => "Unauthorized",
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not found",
            Self::MethodNotAllowed => "Method not allowed",
            Self::Conflict => "Conflict",
            Self::PayloadTooLarge => "Payload too large",
            Self::RequestHeaderFieldsTooLarge => "Request headers too large",
            Self::RequestTimeout => "Request timeout",
            Self::TooManyRequests => "Too many requests",
            Self::ServiceUnavailable => "Service unavailable",
            Self::UnsupportedMediaType => "Unsupported media type",
            Self::InternalServerError => "Internal server error",
        }
    }
}

/// Stable public HTTP error mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicHttpError {
    status: StatusCode,
    code: ErrorCode,
}

impl PublicHttpError {
    /// Creates a stable public HTTP error from a predefined public code.
    pub const fn from_code(code: ErrorCode) -> Self {
        Self {
            status: code.http_status(),
            code,
        }
    }

    /// Returns the stable transport status.
    pub const fn status(self) -> StatusCode {
        self.status
    }

    /// Returns the stable public code.
    pub const fn code(self) -> ErrorCode {
        self.code
    }

    /// Converts the mapping into a JSON transport response.
    pub fn into_response(self) -> JsonErrorResponse {
        JsonErrorResponse::from_public_error(self)
    }
}

/// Trait for mapping internal errors through a stable public HTTP error layer.
///
/// Implementations must return only stable, public classifications. They must
/// not expose internal messages, downstream server names, request bodies,
/// credentials, stack traces, or other diagnostic detail through the public
/// response. Rich internal detail belongs in typed errors and safe structured
/// logs, not in transport contracts.
pub trait ToHttpErrorResponse {
    /// Returns the stable public HTTP error classification.
    fn public_http_error(&self) -> PublicHttpError;

    /// Builds the stable public JSON error response.
    fn to_http_error_response(&self) -> JsonErrorResponse {
        self.public_http_error().into_response()
    }
}

#[cfg(test)]
mod tests {
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
}
