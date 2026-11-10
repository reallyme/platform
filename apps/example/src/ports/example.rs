// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Typed collection of example app ports.
///
/// The example app has no downstream dependencies. Applications with downstream
/// capabilities define typed ports and fail closed when required bindings are absent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExamplePorts {
    /// Private field keeps construction behind the port-collection API.
    _ports: (),
}

impl ExamplePorts {
    /// Constructs the empty example app port collection.
    pub const fn unconfigured() -> Self {
        Self { _ports: () }
    }
}
