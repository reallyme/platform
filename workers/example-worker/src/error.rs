// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;
use thiserror::Error;
use worker::{Response, Result};

use crate::response::stable_error_response;

pub(crate) const INTERNAL_ERROR_MESSAGE: &str = "Internal server error";
pub(crate) const METHOD_NOT_ALLOWED_MESSAGE: &str = "Method not allowed";
pub(crate) const NOT_FOUND_MESSAGE: &str = "Not found";
pub(crate) const UNAVAILABLE_MESSAGE: &str = "Service unavailable";

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExampleWorkerHostError {
    #[error("example worker app config is invalid")]
    InvalidCheckedInConfig,
    #[error("example app is unavailable")]
    AppUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkerPublicErrorCode {
    NotFound,
    MethodNotAllowed,
    ServiceUnavailable,
    Internal,
}

pub(crate) fn map_worker_error(error: ExampleWorkerHostError) -> Result<Response> {
    match error {
        ExampleWorkerHostError::InvalidCheckedInConfig => {
            stable_error_response(WorkerPublicErrorCode::Internal, INTERNAL_ERROR_MESSAGE, 500)
        }
        ExampleWorkerHostError::AppUnavailable => stable_error_response(
            WorkerPublicErrorCode::ServiceUnavailable,
            UNAVAILABLE_MESSAGE,
            503,
        ),
    }
}
