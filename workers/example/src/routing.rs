// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::OnceLock;
use std::time::Duration;

use buffa::Message as _;
use example_app::app::{ExampleAppContext, context_from_config_document};
use example_app::ports::ExamplePorts;
use futures_util::StreamExt;
use reallyme_app_kit::AppHealthStatus;
use reallyme_example_contract::generated::proto::reallyme::example::v1::HelloRequest;
use worker::{Env, Method, Request, Response, ResponseBuilder, Result};

use crate::app_adapter::{EXAMPLE_WORKER_HELLO_PATH, ExampleWorkerMethod, handle_worker_request};
use crate::error::{
    ExampleWorkerHostError, INTERNAL_ERROR_MESSAGE, INVALID_REQUEST_MESSAGE, NOT_FOUND_MESSAGE,
    UNAVAILABLE_MESSAGE, WorkerConnectErrorCode, WorkerPublicErrorCode, map_worker_connect_error,
    map_worker_error,
};
use crate::model::WorkerHealthResponse;
use crate::response::{
    WorkerRouteResponse, connect_error_response, connect_unsupported_media_type_response,
    method_not_allowed_response, stable_error_response,
};

const APP_CONFIG_BINDING: &str = "EXAMPLE_APP_CONFIG_JSONC";
const MAX_CONNECT_REQUEST_BYTES: usize = 64 * 1024;

struct ExampleWorkerState {
    context: ExampleAppContext,
    allowed_origins: Vec<String>,
}

static APP_STATE: OnceLock<core::result::Result<ExampleWorkerState, ExampleWorkerHostError>> =
    OnceLock::new();

pub(crate) async fn route_worker_request(req: &mut Request, env: &Env) -> Result<Response> {
    let origin = req.headers().get("origin")?;
    let allowed = origin.as_deref().and_then(|value| {
        example_state(env)
            .ok()
            .and_then(|state| allowed_origin(&state.allowed_origins, value))
    });

    if let Some(allowed_origin) = allowed
        && req.method() == Method::Options
    {
        let requested_method = req.headers().get("access-control-request-method")?;
        if let Some(requested_method) = requested_method.as_deref()
            && Some(requested_method) == allowed_method_for_path(req.path().as_str())
        {
            let mut response = ResponseBuilder::new().with_status(204).empty();
            response
                .headers_mut()
                .set("access-control-allow-methods", requested_method)?;
            response.headers_mut().set(
                "access-control-allow-headers",
                "content-type, connect-timeout-ms",
            )?;
            apply_cors_headers(&mut response, allowed_origin)?;
            return Ok(response);
        }
    }

    let mut response = route_worker_request_inner(req, env).await?;
    if origin.is_some() {
        response.headers_mut().set("vary", "origin")?;
    }
    if let Some(allowed_origin) = allowed {
        apply_cors_headers(&mut response, allowed_origin)?;
    }
    Ok(response)
}

fn apply_cors_headers(response: &mut Response, origin: &str) -> Result<()> {
    response
        .headers_mut()
        .set("access-control-allow-origin", origin)?;
    response.headers_mut().set("vary", "origin")?;
    Ok(())
}

pub(crate) fn allowed_origin<'a>(allowed: &'a [String], requested: &str) -> Option<&'a str> {
    allowed
        .iter()
        .find(|origin| origin.as_str() == requested)
        .map(String::as_str)
}

pub(crate) fn allowed_method_for_path(path: &str) -> Option<&'static str> {
    match path {
        EXAMPLE_WORKER_HELLO_PATH | "/healthz" | "/readyz" => Some("GET"),
        reallyme_example_contract::EXAMPLE_HELLO_RPC_PATH => Some("POST"),
        _ => None,
    }
}

async fn route_worker_request_inner(req: &mut Request, env: &Env) -> Result<Response> {
    let method = req.method();
    let path = req.path();

    if path == reallyme_example_contract::EXAMPLE_HELLO_RPC_PATH && method == Method::Post {
        return route_connect_request(req, env).await;
    }
    if is_known_app_path(path.as_str()) {
        let allow = if path == EXAMPLE_WORKER_HELLO_PATH {
            "GET"
        } else {
            "POST"
        };
        if (path == EXAMPLE_WORKER_HELLO_PATH && method != Method::Get)
            || (path != EXAMPLE_WORKER_HELLO_PATH && method != Method::Post)
        {
            return method_not_allowed_response(allow);
        }
        let context = match example_context(env) {
            Ok(context) => context,
            Err(error) => return map_worker_error(error),
        };
        return match route_example_app(context, method_to_worker_method(&method), path.as_str()) {
            Ok(Some(response)) => response.into_worker_response(),
            Ok(None) => method_not_allowed_response(allow),
            Err(error) => map_worker_error(error),
        };
    }

    route_operational(method, path.as_str(), env)
}

async fn route_connect_request(req: &mut Request, env: &Env) -> Result<Response> {
    let content_type = match req.headers().get("content-type") {
        Ok(Some(value)) => value,
        Ok(None) | Err(_) => return connect_unsupported_media_type_response(),
    };
    if !is_connect_content_type(content_type.as_str()) {
        return connect_unsupported_media_type_response();
    }
    let timeout_values = match req.headers().get_all("connect-timeout-ms") {
        Ok(values) => values,
        Err(_) => {
            return connect_error_response(
                WorkerConnectErrorCode::InvalidArgument,
                INVALID_REQUEST_MESSAGE,
                400,
            );
        }
    };
    let deadline = match parse_connect_timeout_values(&timeout_values) {
        Ok(deadline) => deadline,
        Err(ConnectTimeoutError::Invalid) => {
            return connect_error_response(
                WorkerConnectErrorCode::InvalidArgument,
                INVALID_REQUEST_MESSAGE,
                400,
            );
        }
    };
    let body = match read_bounded_connect_body(req).await {
        Ok(body) => body,
        Err(ConnectBodyError::TooLarge) => {
            return connect_error_response(
                WorkerConnectErrorCode::ResourceExhausted,
                INVALID_REQUEST_MESSAGE,
                413,
            );
        }
        Err(ConnectBodyError::Invalid) => {
            return connect_error_response(
                WorkerConnectErrorCode::InvalidArgument,
                INVALID_REQUEST_MESSAGE,
                400,
            );
        }
    };
    if !is_valid_hello_request(&body) {
        return connect_error_response(
            WorkerConnectErrorCode::InvalidArgument,
            INVALID_REQUEST_MESSAGE,
            400,
        );
    }
    let context = match example_context(env) {
        Ok(context) => context,
        Err(error) => return map_worker_connect_error(error),
    };
    match route_example_app_with_deadline(
        context,
        ExampleWorkerMethod::Post,
        reallyme_example_contract::EXAMPLE_HELLO_RPC_PATH,
        deadline,
    ) {
        Ok(Some(response)) => response.into_worker_response(),
        Ok(None) => connect_error_response(
            WorkerConnectErrorCode::Internal,
            INTERNAL_ERROR_MESSAGE,
            500,
        ),
        Err(error) => map_worker_connect_error(error),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectTimeoutError {
    Invalid,
}

pub(crate) fn parse_connect_timeout_values(
    values: &[String],
) -> core::result::Result<Option<Duration>, ConnectTimeoutError> {
    match values {
        [] => Ok(None),
        [value] if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => value
            .parse::<u64>()
            .map(Duration::from_millis)
            .map(Some)
            .map_err(|_| ConnectTimeoutError::Invalid),
        _ => Err(ConnectTimeoutError::Invalid),
    }
}

pub(crate) fn is_connect_content_type(value: &str) -> bool {
    value.eq_ignore_ascii_case("application/proto")
}

pub(crate) fn is_valid_hello_request(body: &[u8]) -> bool {
    let mut input = body;
    HelloRequest::decode(&mut input).is_ok() && input.is_empty()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectBodyError {
    Invalid,
    TooLarge,
}

async fn read_bounded_connect_body(
    req: &mut Request,
) -> core::result::Result<Vec<u8>, ConnectBodyError> {
    let declared_length = req
        .headers()
        .get("content-length")
        .map_err(|_| ConnectBodyError::Invalid)?
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| ConnectBodyError::Invalid)
        })
        .transpose()?;
    if declared_length.is_some_and(|length| length > MAX_CONNECT_REQUEST_BYTES) {
        return Err(ConnectBodyError::TooLarge);
    }

    let mut stream = match req.stream() {
        Ok(stream) => stream,
        Err(_) if declared_length.unwrap_or(0) == 0 => return Ok(Vec::new()),
        Err(_) => return Err(ConnectBodyError::Invalid),
    };
    let mut body = Vec::with_capacity(declared_length.unwrap_or(0));
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ConnectBodyError::Invalid)?;
        let new_length = body
            .len()
            .checked_add(chunk.len())
            .ok_or(ConnectBodyError::TooLarge)?;
        if new_length > MAX_CONNECT_REQUEST_BYTES {
            return Err(ConnectBodyError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    if declared_length.is_some_and(|length| length != body.len()) {
        return Err(ConnectBodyError::Invalid);
    }
    Ok(body)
}

pub(crate) fn route_example_app(
    context: &ExampleAppContext,
    method: ExampleWorkerMethod,
    path: &str,
) -> core::result::Result<Option<WorkerRouteResponse>, ExampleWorkerHostError> {
    route_example_app_with_deadline(context, method, path, None)
}

pub(crate) fn route_example_app_with_deadline(
    context: &ExampleAppContext,
    method: ExampleWorkerMethod,
    path: &str,
    deadline: Option<Duration>,
) -> core::result::Result<Option<WorkerRouteResponse>, ExampleWorkerHostError> {
    handle_worker_request(context, method, path, deadline)
}

fn route_operational(method: Method, path: &str, env: &Env) -> Result<Response> {
    match (method, path) {
        (Method::Get, "/healthz") => Response::from_json(&WorkerHealthResponse::serving()),
        (Method::Get, "/readyz") => match example_context(env) {
            Ok(context)
                if example_app::app::app_health(context).status() == AppHealthStatus::Ready =>
            {
                Response::from_json(&WorkerHealthResponse::serving())
            }
            Ok(_) | Err(_) => stable_error_response(
                WorkerPublicErrorCode::ServiceUnavailable,
                UNAVAILABLE_MESSAGE,
                503,
            ),
        },
        (_, known_path) if is_known_operational_path(known_path) => {
            method_not_allowed_response("GET")
        }
        _ => stable_error_response(WorkerPublicErrorCode::NotFound, NOT_FOUND_MESSAGE, 404),
    }
}

pub(crate) fn is_known_app_path(path: &str) -> bool {
    path == EXAMPLE_WORKER_HELLO_PATH || path == reallyme_example_contract::EXAMPLE_HELLO_RPC_PATH
}

pub(crate) fn is_known_operational_path(path: &str) -> bool {
    matches!(path, "/healthz" | "/readyz")
}

fn example_context(
    env: &Env,
) -> core::result::Result<&'static ExampleAppContext, ExampleWorkerHostError> {
    example_state(env).map(|state| &state.context)
}

fn example_state(
    env: &Env,
) -> core::result::Result<&'static ExampleWorkerState, ExampleWorkerHostError> {
    APP_STATE
        .get_or_init(|| {
            let config = env
                .var(APP_CONFIG_BINDING)
                .map_err(|_| ExampleWorkerHostError::InvalidConfiguration)?;
            let document = example_app::app::parse_example_app_config_document(&config.to_string())
                .map_err(|_| ExampleWorkerHostError::InvalidConfiguration)?;
            Ok(ExampleWorkerState {
                allowed_origins: document
                    .cors()
                    .allowed_origins()
                    .iter()
                    .map(|origin| origin.as_str().to_owned())
                    .collect(),
                context: context_from_config_document(&document, ExamplePorts::unconfigured()),
            })
        })
        .as_ref()
        .map_err(|error| *error)
}

fn method_to_worker_method(method: &Method) -> ExampleWorkerMethod {
    match method {
        Method::Get => ExampleWorkerMethod::Get,
        Method::Post => ExampleWorkerMethod::Post,
        _ => ExampleWorkerMethod::Other,
    }
}
