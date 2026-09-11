// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! HTTP transport testing helpers built on `axum-test`.
//!
//! These helpers are intended for service and infrastructure tests that need to
//! exercise Axum routes end-to-end without hand-writing low-level request and
//! response plumbing.

pub use axum_test::{
    TestRequest, TestResponse, TestServer, TestServerBuilder, TestServerConfig, Transport,
};
