// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Generic host-neutral app context.
///
/// Apps commonly need the same shape: a host-neutral core plus injected ports.
/// App-kit owns that boring shape so product apps do not copy it. Concrete app
/// crates still own the core type, port traits, state, and use-cases.
#[derive(Debug, Clone)]
pub struct StandardAppContext<TCore, TPorts> {
    core: TCore,
    ports: TPorts,
}

impl<TCore, TPorts> StandardAppContext<TCore, TPorts> {
    /// Constructs an app context from a core and typed ports.
    pub const fn new(core: TCore, ports: TPorts) -> Self {
        Self { core, ports }
    }

    /// Returns the host-neutral app core.
    pub const fn core(&self) -> &TCore {
        &self.core
    }

    /// Returns injected app ports.
    pub const fn ports(&self) -> &TPorts {
        &self.ports
    }
}
