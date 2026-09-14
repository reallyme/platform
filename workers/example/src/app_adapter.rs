// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Worker-host adapter for the example app core.

use example_app::app::{ExampleAppContext, HelloRequest, hello};

use crate::error::ExampleWorkerHostError;
use crate::response::WorkerRouteResponse;

/// HTTP path exposed by the Worker host for the example hello use-case.
pub(crate) const EXAMPLE_WORKER_HELLO_PATH: &str = "/hello";

/// Worker host method classification used before app logic is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExampleWorkerMethod {
    /// HTTP GET.
    Get,
    /// HTTP POST.
    Post,
    /// Any method the Worker host does not route to app behavior.
    Other,
}

/// Routes a Worker request into host-neutral example app behavior.
///
/// The Worker host owns request routing and response shaping. The example app
/// core only receives typed use-case requests and remains unaware of
/// Cloudflare-specific request/response types.
pub(crate) fn handle_worker_request(
    context: &ExampleAppContext,
    method: ExampleWorkerMethod,
    path: &str,
) -> Result<Option<WorkerRouteResponse>, ExampleWorkerHostError> {
    match (method, path) {
        (ExampleWorkerMethod::Get, EXAMPLE_WORKER_HELLO_PATH) => {
            hello_response(context).map(|body| Some(WorkerRouteResponse::PlainHello(body)))
        }
        (ExampleWorkerMethod::Post, reallyme_example_contract::EXAMPLE_HELLO_CONNECT_RPC_PATH) => {
            hello_response(context).map(|body| Some(WorkerRouteResponse::ConnectHello(body)))
        }
        _ => Ok(None),
    }
}

fn hello_response(context: &ExampleAppContext) -> Result<&'static str, ExampleWorkerHostError> {
    hello(context, HelloRequest, None)
        .map(|response| response.body())
        .map_err(|_error| ExampleWorkerHostError::AppUnavailable)
}
