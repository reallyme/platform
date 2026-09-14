// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature-neutral RPC identity metadata for the example app contract.
//!
//! App adapters must import these protobuf-derived constants from the contract
//! crate rather than from each other. That keeps Connect-only, gRPC-only,
//! native-server-only, and Worker builds independently composable.

/// Canonical protobuf service name for the example app.
pub const EXAMPLE_RPC_SERVICE_NAME: &str = "reallyme.example.v1.ExampleService";

/// Canonical protobuf method name for the example hello use-case.
pub const EXAMPLE_HELLO_RPC_METHOD_NAME: &str = "Hello";

/// Canonical Connect/gRPC path for the example hello use-case.
pub const EXAMPLE_HELLO_RPC_PATH: &str = "/reallyme.example.v1.ExampleService/Hello";

/// Backward-compatible alias for the canonical example hello RPC path.
pub const EXAMPLE_HELLO_CONNECT_RPC_PATH: &str = EXAMPLE_HELLO_RPC_PATH;

/// Canonical RPC method path metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExampleRpcMethodPath {
    service_name: &'static str,
    method_name: &'static str,
    path: &'static str,
}

impl ExampleRpcMethodPath {
    /// Constructs static RPC path metadata.
    pub const fn new(
        service_name: &'static str,
        method_name: &'static str,
        path: &'static str,
    ) -> Self {
        Self {
            service_name,
            method_name,
            path,
        }
    }

    /// Returns the fully qualified protobuf service name.
    pub const fn service_name(self) -> &'static str {
        self.service_name
    }

    /// Returns the protobuf method name.
    pub const fn method_name(self) -> &'static str {
        self.method_name
    }

    /// Returns the canonical Connect/gRPC method path.
    pub const fn path(self) -> &'static str {
        self.path
    }
}

/// Canonical example hello RPC path metadata.
pub const EXAMPLE_HELLO_RPC: ExampleRpcMethodPath = ExampleRpcMethodPath::new(
    EXAMPLE_RPC_SERVICE_NAME,
    EXAMPLE_HELLO_RPC_METHOD_NAME,
    EXAMPLE_HELLO_RPC_PATH,
);

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
