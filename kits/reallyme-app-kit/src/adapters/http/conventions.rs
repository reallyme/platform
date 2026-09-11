// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Policy conventions for HTTP app adapters.
//!
//! These constants are documentation-only and do not alter host lifecycle behavior.

/// Host-neutral policy surface for app-owned HTTP adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpAdapterConventions;

impl HttpAdapterConventions {
    /// App adapters should not install process middleware.
    pub const APP_INSTALLS_PROCESS_MIDDLEWARE: bool = false;
    /// HTTP/JSON is not canonical when an RPC contract exists.
    pub const HTTP_JSON_IS_CANONICAL: bool = false;
}
