// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generated protobuf/Connect code boundary.
//!
//! This crate is the long-term home for generated code emitted by `buf
//! generate`. App adapters import generated transport code from this contract
//! crate so DTOs and RPC service traits have one canonical owner.

/// Canonical protobuf package for the example app.
pub const EXAMPLE_PROTO_PACKAGE: &str = "reallyme.example.v1";

/// Generated Connect service stubs for the example app.
#[allow(missing_docs)]
#[cfg(feature = "generated")]
#[path = "generated/connect/mod.rs"]
pub mod connect;

/// Generated Buffa protobuf message and view types for the example app.
#[allow(missing_docs)]
#[cfg(feature = "generated")]
#[path = "generated/buffa/mod.rs"]
pub mod proto;
