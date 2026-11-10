// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

/// Host or transport adapter kind used by an app.
///
/// This enum documents the supported adapter boundary without coupling app
/// core behavior to any concrete runtime. Each kind identifies a transport or
/// host boundary; applications declare only the adapters they implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppAdapterKind {
    /// Host-neutral core behavior.
    Core,
    /// HTTP transport adapter.
    Http,
    /// gRPC transport adapter.
    Grpc,
    /// Connect RPC transport adapter.
    ConnectRpc,
    /// Native ReallyMe server-process host adapter.
    Server,
    /// Cloudflare Workers host adapter.
    Worker,
}
