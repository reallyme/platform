// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Typed collection of example app ports.
///
/// The example app has no downstream ports. Real product apps should
/// replace this with app-specific typed port traits and deterministic
/// fail-closed unconfigured implementations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExamplePorts {
    /// Placeholder marker for explicit typed construction.
    _ports: (),
}

impl ExamplePorts {
    /// Constructs example app ports when no downstream ports are wired yet.
    pub const fn unconfigured() -> Self {
        Self { _ports: () }
    }
}
