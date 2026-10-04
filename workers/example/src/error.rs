// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use example_app::app::ExampleAppErrorKind;
use serde::Serialize;
use thiserror::Error;
use worker::{Response, Result};

use crate::response::{connect_error_response, stable_error_response};

pub(crate) const INTERNAL_ERROR_MESSAGE: &str = "Internal server error";
pub(crate) const METHOD_NOT_ALLOWED_MESSAGE: &str = "Method not allowed";
pub(crate) const NOT_FOUND_MESSAGE: &str = "Not found";
pub(crate) const UNAVAILABLE_MESSAGE: &str = "Service unavailable";
pub(crate) const FORBIDDEN_MESSAGE: &str = "Forbidden";
pub(crate) const INVALID_REQUEST_MESSAGE: &str = "Invalid request";

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExampleWorkerHostError {
    #[error("example worker app config is invalid")]
    InvalidConfiguration,
    #[error("example app call failed")]
    App { kind: ExampleAppErrorKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkerPublicErrorCode {
    NotFound,
    MethodNotAllowed,
    ServiceUnavailable,
    Forbidden,
    InternalServerError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkerConnectErrorCode {
    InvalidArgument,
    PermissionDenied,
    ResourceExhausted,
    DeadlineExceeded,
    Internal,
}

pub(crate) fn map_worker_error(error: ExampleWorkerHostError) -> Result<Response> {
    let (code, message, status) = classify_worker_error(error);
    stable_error_response(code, message, status)
}

pub(crate) fn classify_worker_error(
    error: ExampleWorkerHostError,
) -> (WorkerPublicErrorCode, &'static str, u16) {
    match error {
        ExampleWorkerHostError::InvalidConfiguration => (
            WorkerPublicErrorCode::InternalServerError,
            INTERNAL_ERROR_MESSAGE,
            500,
        ),
        ExampleWorkerHostError::App {
            kind: ExampleAppErrorKind::HelloDisabled,
        } => (WorkerPublicErrorCode::Forbidden, FORBIDDEN_MESSAGE, 403),
        ExampleWorkerHostError::App {
            kind: ExampleAppErrorKind::MetricConfigurationInvalid,
        } => (
            WorkerPublicErrorCode::InternalServerError,
            INTERNAL_ERROR_MESSAGE,
            500,
        ),
        ExampleWorkerHostError::App {
            kind: ExampleAppErrorKind::DeadlineExceeded,
        } => (
            WorkerPublicErrorCode::ServiceUnavailable,
            UNAVAILABLE_MESSAGE,
            503,
        ),
    }
}

pub(crate) fn map_worker_connect_error(error: ExampleWorkerHostError) -> Result<Response> {
    let (code, message, status) = classify_worker_connect_error(error);
    connect_error_response(code, message, status)
}

pub(crate) fn classify_worker_connect_error(
    error: ExampleWorkerHostError,
) -> (WorkerConnectErrorCode, &'static str, u16) {
    match error {
        ExampleWorkerHostError::App {
            kind: ExampleAppErrorKind::HelloDisabled,
        } => (
            WorkerConnectErrorCode::PermissionDenied,
            FORBIDDEN_MESSAGE,
            403,
        ),
        ExampleWorkerHostError::App {
            kind: ExampleAppErrorKind::MetricConfigurationInvalid,
        }
        | ExampleWorkerHostError::InvalidConfiguration => (
            WorkerConnectErrorCode::Internal,
            INTERNAL_ERROR_MESSAGE,
            500,
        ),
        ExampleWorkerHostError::App {
            kind: ExampleAppErrorKind::DeadlineExceeded,
        } => (
            WorkerConnectErrorCode::DeadlineExceeded,
            UNAVAILABLE_MESSAGE,
            504,
        ),
    }
}
