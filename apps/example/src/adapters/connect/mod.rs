// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Connect RPC adapter boundary for the example app.

mod error;
mod router;
mod service;

pub use reallyme_example_contract::generated::{connect, proto};

pub use reallyme_app_kit::ConnectAdapterConventions;
pub use reallyme_example_contract::{
    EXAMPLE_HELLO_CONNECT_RPC_PATH, EXAMPLE_HELLO_RPC, EXAMPLE_HELLO_RPC_METHOD_NAME,
    EXAMPLE_HELLO_RPC_PATH, EXAMPLE_RPC_SERVICE_NAME, ExampleRpcMethodPath,
};
pub use router::connect_router;
pub use service::ExampleConnectService;
