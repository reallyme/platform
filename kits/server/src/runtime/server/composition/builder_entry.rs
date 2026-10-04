// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entry point for constructing a validated server runtime.

use super::{ServerRuntime, ServerRuntimeBuilder};

impl ServerRuntime {
    /// Creates a new server runtime builder.
    pub fn builder() -> ServerRuntimeBuilder {
        ServerRuntimeBuilder::default()
    }
}
