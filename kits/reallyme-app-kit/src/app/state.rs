// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Generic host-neutral app state wrapper.
///
/// Most apps begin with validated app-owned config as their only state. App-kit
/// owns that boring container so new apps do not copy one-field state structs.
/// Apps can replace this with app-specific state later when they truly own
/// additional host-neutral resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardAppState<TConfig> {
    config: TConfig,
}

impl<TConfig> StandardAppState<TConfig> {
    /// Constructs app state from validated app config.
    pub const fn new(config: TConfig) -> Self {
        Self { config }
    }

    /// Returns the validated app config.
    pub const fn config(&self) -> &TConfig {
        &self.config
    }
}
