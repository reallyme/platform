// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use example_app::app::{ExampleAppContext, context_from_config_document};
use example_app::ports::ExamplePorts;
use worker::{Method, Response, Result};

use crate::app_adapter::{EXAMPLE_WORKER_HELLO_PATH, ExampleWorkerMethod, handle_worker_request};
use crate::error::{
    ExampleWorkerHostError, NOT_FOUND_MESSAGE, WorkerPublicErrorCode, map_worker_error,
};
use crate::model::{WorkerHealthResponse, WorkerVersionResponse};
use crate::response::{
    WorkerRouteResponse, method_not_allowed_response, metrics_response, stable_error_response,
};

const EXAMPLE_APP_CONFIG_JSONC: &str = include_str!("../../../apps/example/config/local.jsonc");

pub(crate) fn route_worker_request(method: Method, path: &str) -> Result<Response> {
    match route_example_app(method_to_worker_method(&method), path) {
        Ok(Some(response)) => response.into_worker_response(),
        Ok(None) if is_known_app_path(path) => method_not_allowed_response(),
        Ok(None) => route_operational(method, path),
        Err(error) => map_worker_error(error),
    }
}

pub(crate) fn route_example_app(
    method: ExampleWorkerMethod,
    path: &str,
) -> core::result::Result<Option<WorkerRouteResponse>, ExampleWorkerHostError> {
    let context = example_context()?;

    handle_worker_request(&context, method, path)
}

fn route_operational(method: Method, path: &str) -> Result<Response> {
    match (method, path) {
        (Method::Get, "/healthz") | (Method::Get, "/readyz") => {
            Response::from_json(&WorkerHealthResponse::serving())
        }
        (Method::Get, "/version") => Response::from_json(&WorkerVersionResponse::current()),
        (Method::Get, "/metrics") => metrics_response(),
        (_, known_path) if is_known_operational_path(known_path) => method_not_allowed_response(),
        _ => stable_error_response(WorkerPublicErrorCode::NotFound, NOT_FOUND_MESSAGE, 404),
    }
}

pub(crate) fn is_known_app_path(path: &str) -> bool {
    path == EXAMPLE_WORKER_HELLO_PATH || path == reallyme_example_contract::EXAMPLE_HELLO_RPC_PATH
}

pub(crate) fn is_known_operational_path(path: &str) -> bool {
    matches!(path, "/healthz" | "/readyz" | "/version" | "/metrics")
}

fn example_context() -> core::result::Result<ExampleAppContext, ExampleWorkerHostError> {
    let document = example_app::app::parse_example_app_config_document(EXAMPLE_APP_CONFIG_JSONC)
        .map_err(|_error| ExampleWorkerHostError::InvalidCheckedInConfig)?;

    Ok(context_from_config_document(
        &document,
        ExamplePorts::unconfigured(),
    ))
}

fn method_to_worker_method(method: &Method) -> ExampleWorkerMethod {
    match method {
        Method::Get => ExampleWorkerMethod::Get,
        Method::Post => ExampleWorkerMethod::Post,
        _ => ExampleWorkerMethod::Other,
    }
}
