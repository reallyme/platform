// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

/// Host or transport adapter kind used by an app.
///
/// This enum documents the supported adapter boundary without coupling app
/// core behavior to any concrete runtime. New host kinds should be added only
/// when a real adapter exists or is actively being built.
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
    /// Future Cloudflare Workers host adapter.
    Worker,
}
