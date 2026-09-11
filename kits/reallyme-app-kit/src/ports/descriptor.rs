// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppPortName;

/// Host-neutral app port descriptor.
///
/// Apps define which ports they require; app-kit defines the common descriptor
/// shape used by hosts, tests, and future Workers adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPortDescriptor {
    name: AppPortName,
    required: bool,
}

impl AppPortDescriptor {
    /// Constructs a port descriptor.
    pub const fn new(name: AppPortName, required: bool) -> Self {
        Self { name, required }
    }

    /// Returns the port name.
    pub const fn name(&self) -> &AppPortName {
        &self.name
    }

    /// Returns whether the port is required for startup.
    pub const fn required(&self) -> bool {
        self.required
    }
}
