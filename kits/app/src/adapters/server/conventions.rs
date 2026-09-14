// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Policy conventions for server-host app adapters.
//!
//! These constants document adapter boundaries only; runtime ownership remains with
//! the host.

/// Host-neutral policy surface for server-host adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerAdapterConventions;

impl ServerAdapterConventions {
    /// App server adapters should not own process lifecycle.
    pub const APP_OWNS_PROCESS_LIFECYCLE: bool = false;
}
