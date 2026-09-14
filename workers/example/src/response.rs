// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use buffa::Message as _;
use reallyme_example_contract::generated::proto::reallyme::example::v1::HelloResponse;
use worker::{Response, ResponseBuilder, Result};

use crate::error::{METHOD_NOT_ALLOWED_MESSAGE, WorkerPublicErrorCode};
use crate::model::{WorkerErrorBody, WorkerErrorEnvelope, WorkerHelloResponse};

const CACHE_CONTROL_NO_STORE: &str = "no-store";
const CONTENT_SECURITY_POLICY_API: &str =
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
const METRICS_BODY: &str = "# Worker metrics are exported by Cloudflare Workers observability.\n";
const METRICS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerRouteResponse {
    PlainHello(&'static str),
    ConnectHello(&'static str),
}

impl WorkerRouteResponse {
    pub(crate) fn into_worker_response(self) -> Result<Response> {
        match self {
            Self::PlainHello(message) => Response::from_json(&WorkerHelloResponse {
                message: message.trim_end_matches('\n'),
            }),
            Self::ConnectHello(message) => Ok(ResponseBuilder::new()
                .with_header("content-type", "application/proto")?
                .fixed(
                    HelloResponse {
                        message: message.trim_end_matches('\n').to_owned(),
                        ..Default::default()
                    }
                    .encode_to_vec(),
                )),
        }
    }
}

pub(crate) fn metrics_response() -> Result<Response> {
    Ok(ResponseBuilder::new()
        .with_header("content-type", METRICS_CONTENT_TYPE)?
        .fixed(METRICS_BODY.as_bytes().to_vec()))
}

pub(crate) fn method_not_allowed_response() -> Result<Response> {
    stable_error_response(
        WorkerPublicErrorCode::MethodNotAllowed,
        METHOD_NOT_ALLOWED_MESSAGE,
        405,
    )
}

pub(crate) fn stable_error_response(
    code: WorkerPublicErrorCode,
    message: &'static str,
    status: u16,
) -> Result<Response> {
    Response::from_json(&WorkerErrorEnvelope {
        error: WorkerErrorBody { code, message },
    })
    .map(|response| response.with_status(status))
}

pub(crate) fn with_standard_worker_headers(mut response: Response) -> Result<Response> {
    let headers = response.headers_mut();

    // Keep host-level hardening close to the Worker host boundary. App cores
    // should not know whether a response is served by Cloudflare Workers,
    // reallyme-server, or a future host. HSTS is intentionally left to
    // Cloudflare zone/origin policy because this example also runs under
    // plaintext `wrangler dev`.
    headers.set("x-content-type-options", "nosniff")?;
    headers.set("referrer-policy", "no-referrer")?;
    headers.set("content-security-policy", CONTENT_SECURITY_POLICY_API)?;
    headers.set("cache-control", CACHE_CONTROL_NO_STORE)?;

    Ok(response)
}
