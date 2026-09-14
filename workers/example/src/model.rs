// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;

use crate::error::WorkerPublicErrorCode;

pub(crate) const HEALTH_STATUS_OK: &str = "ok";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct WorkerHelloResponse {
    pub(crate) message: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct WorkerErrorEnvelope {
    pub(crate) error: WorkerErrorBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct WorkerErrorBody {
    pub(crate) code: WorkerPublicErrorCode,
    pub(crate) message: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct WorkerHealthResponse {
    status: &'static str,
}

impl WorkerHealthResponse {
    pub(crate) const fn serving() -> Self {
        Self {
            status: HEALTH_STATUS_OK,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct WorkerVersionResponse {
    app_name: &'static str,
    app_version: &'static str,
    host: &'static str,
}

impl WorkerVersionResponse {
    pub(crate) const fn current() -> Self {
        Self {
            app_name: "example-app",
            app_version: env!("CARGO_PKG_VERSION"),
            host: "cloudflare-workers",
        }
    }
}
