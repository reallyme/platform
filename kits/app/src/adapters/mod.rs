// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral adapter conventions.

pub mod connect;
pub mod grpc;
pub mod http;
pub mod server;
pub mod workers;

pub use connect::{
    ConnectAdapterConventions, ConnectCodeGenerationWorkflow, ConnectJsonCompatibilityPolicy,
    ConnectProtocolEncoding, ConnectPublicEncodingPolicy, ConnectSchemaSource,
};
pub use grpc::GrpcAdapterConventions;
pub use http::HttpAdapterConventions;
pub use server::ServerAdapterConventions;
pub use workers::WorkersAdapterConventions;
