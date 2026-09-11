// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! Contract crate for the example app.
//!
//! This crate owns app DTOs and host-neutral port traits. Runtime hosts and
//! BFFs may depend on this contract without depending on the example app
//! implementation.

/// Example app host-neutral DTOs.
pub mod dto;
/// Example app typed contract errors.
pub mod error;
/// Generated protobuf/Connect boundary.
pub mod generated;
/// Example app host-neutral port traits.
pub mod port;
/// Feature-neutral example app RPC metadata.
pub mod rpc;

pub use dto::{HelloRequest, HelloResponse};
pub use error::{ExampleContractError, ExampleContractErrorKind};
pub use port::{ExamplePort, ExamplePortCall, InProcessExamplePortDescriptor};
pub use rpc::{
    EXAMPLE_HELLO_CONNECT_RPC_PATH, EXAMPLE_HELLO_RPC, EXAMPLE_HELLO_RPC_METHOD_NAME,
    EXAMPLE_HELLO_RPC_PATH, EXAMPLE_RPC_SERVICE_NAME, ExampleRpcMethodPath,
};
