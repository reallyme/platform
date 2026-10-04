// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use worker::{Context, Env, Request, Response, Result, event};

use crate::response::with_standard_worker_headers;
use crate::routing::route_worker_request;

/// Cloudflare Workers fetch entrypoint.
#[event(fetch)]
pub async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    route_worker_request(&mut req, &env)
        .await
        .and_then(with_standard_worker_headers)
}
