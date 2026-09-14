// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Policy conventions for Cloudflare Workers app adapters.
//!
//! These constants document compile-time adapter intent and do not alter runtime.

/// Host-neutral policy surface for Workers adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkersAdapterConventions;

impl WorkersAdapterConventions {
    /// Workers adapters should not fork product behavior.
    pub const FORKS_PRODUCT_BEHAVIOR: bool = false;
    /// Workers adapters should reuse host-neutral app core.
    pub const REUSES_APP_CORE: bool = true;
    /// Workers adapters should not own process lifecycle.
    pub const OWNS_PROCESS_LIFECYCLE: bool = false;
    /// Workers hosts should compile for Wasm.
    pub const REQUIRES_WASM32_UNKNOWN_UNKNOWN: bool = true;
    /// Local development should use Wrangler.
    pub const USES_WRANGLER_DEV: bool = true;
    /// app-kit should not depend directly on workers-rs.
    pub const APP_KIT_DEPENDS_ON_WORKERS_RS: bool = false;

    /// Returns whether Workers adapters should fork product behavior.
    pub const fn forks_product_behavior(self) -> bool {
        Self::FORKS_PRODUCT_BEHAVIOR
    }

    /// Returns whether Workers adapters should reuse host-neutral app core.
    pub const fn reuses_app_core(self) -> bool {
        Self::REUSES_APP_CORE
    }

    /// Returns whether Workers adapters own native server-process lifecycle.
    pub const fn owns_process_lifecycle(self) -> bool {
        Self::OWNS_PROCESS_LIFECYCLE
    }

    /// Returns whether a concrete Worker host should compile for Wasm.
    pub const fn requires_wasm32_unknown_unknown(self) -> bool {
        Self::REQUIRES_WASM32_UNKNOWN_UNKNOWN
    }

    /// Returns whether local development should use Wrangler.
    pub const fn uses_wrangler_dev(self) -> bool {
        Self::USES_WRANGLER_DEV
    }

    /// Returns whether app-kit should depend on workers-rs.
    pub const fn app_kit_depends_on_workers_rs(self) -> bool {
        Self::APP_KIT_DEPENDS_ON_WORKERS_RS
    }
}
