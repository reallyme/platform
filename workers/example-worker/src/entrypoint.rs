// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use worker::{Context, Env, Request, Response, Result, event};

use crate::response::with_standard_worker_headers;
use crate::routing::route_worker_request;

/// Cloudflare Workers fetch entrypoint.
#[event(fetch)]
pub async fn fetch(req: Request, _env: Env, _ctx: Context) -> Result<Response> {
    route_worker_request(req.method(), req.path().as_str()).and_then(with_standard_worker_headers)
}
