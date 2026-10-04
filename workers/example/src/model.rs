// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;

use crate::error::{WorkerConnectErrorCode, WorkerPublicErrorCode};

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
pub(crate) struct WorkerConnectErrorBody {
    pub(crate) code: WorkerConnectErrorCode,
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
