// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use super::error::{ErrorCode, PublicHttpError};
use crate::transport::RequestId;

/// Machine- and human-readable error body returned by JSON endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonErrorBody {
    code: ErrorCode,
    message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<RequestId>,
}

impl JsonErrorBody {
    /// Returns the stable machine-readable public code.
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    /// Returns the stable public message.
    pub const fn message(&self) -> &'static str {
        self.message
    }

    /// Returns the request identifier attached for correlation, if any.
    pub const fn request_id(&self) -> Option<RequestId> {
        self.request_id
    }
}

/// Stable JSON error envelope returned by infrastructure endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonErrorEnvelope {
    error: JsonErrorBody,
}

impl JsonErrorEnvelope {
    /// Returns the enclosed error body.
    pub const fn error(&self) -> &JsonErrorBody {
        &self.error
    }
}

/// Stable JSON error envelope paired with an HTTP status code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonErrorResponse {
    status: StatusCode,
    body: JsonErrorEnvelope,
}

impl JsonErrorResponse {
    /// Creates a stable JSON error response from a public HTTP error mapping.
    pub fn from_public_error(error: PublicHttpError) -> Self {
        Self {
            status: error.status(),
            body: JsonErrorEnvelope {
                error: JsonErrorBody {
                    code: error.code(),
                    message: error.code().public_message(),
                    request_id: None,
                },
            },
        }
    }

    /// Creates a stable bad-request response.
    pub fn bad_request() -> Self {
        Self::from_public_error(PublicHttpError::from_code(ErrorCode::BadRequest))
    }

    /// Creates a stable payload-too-large response.
    pub fn payload_too_large() -> Self {
        Self::from_public_error(PublicHttpError::from_code(ErrorCode::PayloadTooLarge))
    }

    /// Creates a stable request-header-fields-too-large response.
    pub fn request_headers_too_large() -> Self {
        Self::from_public_error(PublicHttpError::from_code(
            ErrorCode::RequestHeaderFieldsTooLarge,
        ))
    }

    /// Creates a stable request-timeout response.
    pub fn request_timeout() -> Self {
        Self::from_public_error(PublicHttpError::from_code(ErrorCode::RequestTimeout))
    }

    /// Creates a stable service-unavailable response.
    pub fn service_unavailable() -> Self {
        Self::from_public_error(PublicHttpError::from_code(ErrorCode::ServiceUnavailable))
    }

    /// Creates a stable internal-server-error response.
    pub fn internal_server_error() -> Self {
        Self::from_public_error(PublicHttpError::from_code(ErrorCode::InternalServerError))
    }

    /// Creates a stable not-found response.
    pub fn not_found() -> Self {
        Self::from_public_error(PublicHttpError::from_code(ErrorCode::NotFound))
    }

    /// Creates a stable method-not-allowed response.
    pub fn method_not_allowed() -> Self {
        Self::from_public_error(PublicHttpError::from_code(ErrorCode::MethodNotAllowed))
    }

    /// Attaches the request identifier that should be returned to callers.
    pub fn with_request_id(mut self, request_id: RequestId) -> Self {
        self.body.error.request_id = Some(request_id);
        self
    }

    /// Attaches the request identifier if one is available.
    pub fn with_optional_request_id(self, request_id: Option<RequestId>) -> Self {
        match request_id {
            Some(request_id) => self.with_request_id(request_id),
            None => self,
        }
    }

    /// Returns the HTTP status code.
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns the serialized body.
    pub const fn body(&self) -> &JsonErrorEnvelope {
        &self.body
    }
}

impl IntoResponse for JsonErrorResponse {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
