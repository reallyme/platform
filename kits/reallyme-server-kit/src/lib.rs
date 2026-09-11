// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! Shared production infrastructure for ReallyMe Rust server processes and apps.
//!
//! `reallyme-server-kit` provides platform concerns that every server process
//! and hosted runtime app should solve consistently:
//!
//! - configuration loading and validation
//! - startup identity validation
//! - graceful shutdown coordination
//! - readiness and liveness state handling
//! - tracing and metrics bootstrap
//! - HTTP and optional standalone tonic gRPC cross-cutting conventions
//! - managed background task lifecycle
//! - authentication and authorization extension points
//! - build and version metadata helpers
//!
//! The crate is intentionally narrow. Product logic, route definitions, storage
//! schemas, and business permissions belong in consuming crates.

/// Tailscale ACL tag validation and matching helpers.
pub mod acl_tags;
/// Authentication extension points.
pub mod authn;
/// Authorization extension points.
pub mod authz;
/// Environment-backed configuration helpers.
pub mod config;
/// gRPC transport conventions and status mapping helpers.
#[cfg(feature = "tonic-grpc")]
pub mod grpc;
/// Readiness and liveness state primitives.
pub mod health;
/// HTTP transport conventions and middleware helpers.
#[cfg(feature = "http")]
pub mod http;
/// Tracing and metrics bootstrap helpers.
pub mod observability;
/// Reusable server runtime orchestration.
#[cfg(feature = "http")]
pub mod runtime;
/// Graceful shutdown primitives.
pub mod shutdown;
/// Service startup identity validation.
pub mod startup;
/// Tailscale-backed service discovery primitives.
pub mod tailscale;
/// Managed background task lifecycle helpers.
pub mod task;
/// Transport-neutral correlation primitives.
pub mod transport;
/// Build and version metadata helpers.
pub mod version;
