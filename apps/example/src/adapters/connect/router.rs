// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;

use connectrpc::Router;
use reallyme_example_contract::generated::connect::reallyme::example::v1::ExampleServiceExt;

use crate::app::ExampleAppContext;

use super::service::ExampleConnectService;

/// Builds the app-owned Connect router.
///
/// The example app carries a real Buf/protobuf contract, but generated Connect
/// handlers remain transport adapters only. The app core stays host-neutral and
/// the native server host owns the process lifecycle.
pub fn connect_router(context: ExampleAppContext) -> Router {
    Arc::new(ExampleConnectService::new(context)).register(Router::new())
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
