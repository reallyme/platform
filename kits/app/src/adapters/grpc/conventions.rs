// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Policy conventions for gRPC app adapters.
//!
//! These constants are documentation-only and do not alter host lifecycle
//! behavior.

/// Host-neutral policy surface for app-owned gRPC adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrpcAdapterConventions;

impl GrpcAdapterConventions {
    /// App adapters should not start listeners.
    pub const APP_STARTS_LISTENER: bool = false;
    /// App adapters should not own reflection policy.
    pub const APP_OWNS_REFLECTION_POLICY: bool = false;
    /// App adapters should not expose internal errors.
    pub const APP_EXPOSES_INTERNAL_ERRORS: bool = false;
}
